// Estado global da UI (Svelte 5 runes). Uma instância, importada onde precisar.

import { api, errorMessage } from "./api";
import type {
  AccountStatus,
  LocalPlaylist,
  Lyrics,
  PlayerSnapshot,
  Playlist,
  QueueSlot,
  Source,
  SourceResults,
  Track,
} from "./types";
import { SOURCE_LABEL, playlistUrl } from "./types";

export type Tab = "search" | "library";
export type Filter = Source | "all";
export type Panel = "now" | "queue" | "friends" | "profile";

/** Lista aberta na biblioteca: uma playlist de um serviço, uma criada no GlobalMusic ou as curtidas. */
export interface OpenedPlaylist {
  kind: "playlist" | "favorites" | "local";
  id: string;
  title: string;
  /** `null` = lista do próprio GlobalMusic (várias fontes). */
  source: Source | null;
  artworkUrl: string | null;
  /** Link público (para compartilhar) de uma playlist de serviço. */
  link?: string | null;
  tracks: Track[];
  loading: boolean;
  error: string | null;
}

/** Letra da faixa atual (carregada quando a pessoa abre a letra). */
export interface LyricsState {
  /** De qual faixa é (fonte:id): trocou a música, carrega de novo. */
  key: string;
  loading: boolean;
  data: Lyrics | null;
  error: string | null;
}

/** Janela "Escolher versão do YouTube". */
export interface VersionPicker {
  track: Track;
  loading: boolean;
  current: string | null;
  options: Track[];
  error: string | null;
}

/** Menu do botão direito sobre uma faixa. */
export interface ContextMenu {
  track: Track;
  x: number;
  y: number;
}

/** O que está sob o menu de playlist: uma do GlobalMusic ou uma de serviço (Spotify, YouTube Music…). */
export type PlaylistTarget = { kind: "local"; playlist: LocalPlaylist } | { kind: "service"; playlist: Playlist };

/** Menu do botão direito sobre uma playlist. */
export interface PlaylistMenu {
  target: PlaylistTarget;
  x: number;
  y: number;
}

const EMPTY_PLAYER: PlayerSnapshot = {
  current: null,
  route: null,
  context: [],
  contextIndex: null,
  contextName: null,
  upNext: [],
  playing: false,
  loading: false,
  positionMs: 0,
  volume: 0.8,
  version: 0,
};
const TICK_MS = 500;
/** Voltando ao app depois disso, a playlist aberta e a biblioteca são lidas de novo (a pessoa pode ter
 *  curtido ou mudado algo no site do serviço enquanto isso). */
const STALE_AFTER_MS = 20_000;
const TOAST_MS = 5000;

/** Intercala as fontes (A1, B1, C1, A2…) para "Todas" não virar três blocos. */
function interleave<T>(groups: T[][]): T[] {
  const out: T[] = [];
  for (let i = 0; groups.some((g) => i < g.length); i++) {
    for (const g of groups) if (i < g.length) out.push(g[i]);
  }
  return out;
}

/** Sugestões da janela "Nova playlist": o que tocou por último e a última busca (ficam neste computador). */
const RECENT_KEY = "globalmusic.recent";
const LAST_SEARCH_KEY = "globalmusic.lastSearch";
const MAX_RECENT = 30;
const MAX_LAST_SEARCH = 20;

function readStored<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : fallback;
  } catch {
    return fallback;
  }
}

function store(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Sem armazenamento (ex.: modo privado no navegador): as sugestões só valem nesta sessão.
  }
}

const sameTrack = (a: Track, b: Track) => a.source === b.source && a.id === b.id;
const trackKey = (t: Track) => `${t.source}:${t.id}`;
/** Quantas faixas do topo de uma lista mandamos preparar (o backend também limita). */
const PREPARE_TOP = 4;

class AppStore {
  tab = $state<Tab>("library");
  /** Painel da esquerda (busca/biblioteca) visível; escondido, sobra só o player. */
  sidebarOpen = $state(true);
  filter = $state<Filter>("all");
  panel = $state<Panel>("now");

  query = $state("");
  searching = $state(false);
  results = $state<SourceResults<Track>[]>([]);
  playlists = $state<SourceResults<Playlist>[]>([]);
  /** Playlists importadas por link (ficam na biblioteca junto com as da pessoa). */
  imported = $state<Playlist[]>([]);
  importing = $state(false);
  /** Playlists criadas no GlobalMusic. */
  locals = $state<LocalPlaylist[]>([]);
  /** Janela "Nova playlist": `null` fechada; aberta, as músicas que já entram marcadas. */
  creating = $state<Track[] | null>(null);
  /** Tocadas por último (a mais recente primeiro), para sugerir numa playlist nova. */
  recent = $state<Track[]>(readStored(RECENT_KEY, []));
  /** Resultados da última busca (sobrevivem a fechar o app), também sugeridos. */
  lastSearch = $state<{ query: string; tracks: Track[] }>(readStored(LAST_SEARCH_KEY, { query: "", tracks: [] }));
  accounts = $state<AccountStatus[]>([]);
  busySource = $state<Source | null>(null);
  notice = $state<string | null>(null);

  player = $state<PlayerSnapshot>(EMPTY_PLAYER);
  positionMs = $state(0);
  /** Última faixa que não pôde tocar (o player já pulou para a próxima). */
  playbackError = $state<string | null>(null);
  /** Lista aberta na biblioteca (tela de detalhe). */
  opened = $state<OpenedPlaylist | null>(null);
  /** Curtidas do GlobalMusic (todas as fontes juntas). */
  favorites = $state<Track[]>([]);
  menu = $state<ContextMenu | null>(null);
  playlistMenu = $state<PlaylistMenu | null>(null);
  /** Janela "Editar playlist" (nome e foto) de uma playlist do GlobalMusic. */
  editing = $state<LocalPlaylist | null>(null);
  /** Aviso curto no rodapé da janela (ex.: "Adicionada à fila"). */
  toast = $state<string | null>(null);
  /** A letra aparece no lugar da capa. */
  showLyrics = $state(false);
  lyrics = $state<LyricsState | null>(null);
  versions = $state<VersionPicker | null>(null);

  #searchSeq = 0;
  #playlistSeq = 0;
  /** Quando a playlist aberta e a biblioteca foram lidas pela última vez. */
  #openedAt = 0;
  #libraryAt = 0;
  #libraryRetry: ReturnType<typeof setTimeout> | undefined;
  #errorTimer: ReturnType<typeof setTimeout> | undefined;
  #toastTimer: ReturnType<typeof setTimeout> | undefined;

  get tracks(): Track[] {
    const groups = this.results.filter((r) => this.filter === "all" || r.source === this.filter).map((r) => r.items);
    return interleave(groups);
  }

  get playlistItems(): Playlist[] {
    const groups = this.playlists.filter((r) => this.filter === "all" || r.source === this.filter).map((r) => r.items);
    const own = interleave(groups);
    // Importadas primeiro (a mais recente no topo); sem repetir uma que a pessoa já tem.
    const extra = this.imported.filter(
      (p) => (this.filter === "all" || p.source === this.filter) && !own.some((o) => o.source === p.source && o.id === p.id),
    );
    return [...extra, ...own];
  }

  isImported(playlist: Playlist) {
    return this.imported.some((p) => p.source === playlist.source && p.id === playlist.id);
  }

  async importPlaylist(link: string) {
    if (!link.trim() || this.importing) return false;
    this.importing = true;
    try {
      const outcome = await api.importPlaylist(link.trim());
      this.imported = outcome.imported;
      this.locals = outcome.locals;
      this.notify(`"${outcome.name}" entrou na sua biblioteca`);
      return true;
    } catch (e) {
      this.notify(errorMessage(e));
      return false;
    } finally {
      this.importing = false;
    }
  }

  async removeImported(playlist: Playlist) {
    this.imported = await api.removeImportedPlaylist(playlist.source, playlist.id);
    this.notify(`"${playlist.name}" saiu da biblioteca`);
  }

  /** A sua fila (o que foi posto à mão): toca antes do resto da lista. */
  get upNext(): { track: Track; slot: QueueSlot }[] {
    return this.player.upNext.map((track, index) => ({ track, slot: { list: "queued", index } }));
  }

  /** O resto da lista de onde a pessoa deu play, depois da posição atual. */
  get contextRest(): { track: Track; slot: QueueSlot }[] {
    const start = (this.player.contextIndex ?? -1) + 1;
    return this.player.context.slice(start).map((track, i) => ({ track, slot: { list: "context", index: start + i } }));
  }

  /**
   * Fontes que **falharam** no recorte atual, para a UI explicar o vazio. Fonte não conectada
   * não conta: é escolha da pessoa (o Perfil mostra o que está conectado), não um erro.
   */
  failures(groups: SourceResults<unknown>[]) {
    return groups.filter(
      (r) => r.error && !r.error.includes("não está conectado") && (this.filter === "all" || r.source === this.filter),
    );
  }

  isFavorite(track: Track) {
    return this.favorites.some((f) => sameTrack(f, track));
  }

  async init() {
    await api.onPlayerState((s) => this.#applyPlayer(s));
    await api.onPlayerNotice((message) => this.notify(message));
    await api.onPlayerError((message) => {
      this.playbackError = message;
      clearTimeout(this.#errorTimer);
      this.#errorTimer = setTimeout(() => (this.playbackError = null), 8000);
    });
    const [player, favorites, imported, locals] = await Promise.all([
      api.playerState(),
      api.favorites(),
      api.importedPlaylists(),
      api.localPlaylists(),
      this.loadAccounts(),
      this.loadLibrary(),
    ]);
    this.favorites = favorites;
    this.imported = imported;
    this.locals = locals;
    this.#applyPlayer(player);
    setInterval(() => this.#tick(), TICK_MS);
  }

  notify(message: string) {
    this.toast = message;
    clearTimeout(this.#toastTimer);
    this.#toastTimer = setTimeout(() => (this.toast = null), TOAST_MS);
  }

  /** Trocar de fonte sai de uma playlist de outra fonte (sempre mostra a biblioteca certa). */
  setFilter(filter: Filter) {
    this.filter = filter;
    const source = this.opened?.source;
    if (this.opened && filter !== "all" && source !== filter) this.closePlaylist();
    if (this.tab === "search") this.prepareVisible(this.tracks);
  }

  /**
   * Faixas que apareceram na tela: o backend já resolve o áudio das primeiras, para o clique
   * não esperar o yt-dlp (~4 s). Uma lista nova substitui a anterior.
   */
  prepareVisible(tracks: Track[]) {
    if (tracks.length) void api.prepare($state.snapshot(tracks.slice(0, PREPARE_TOP))).catch(() => {});
  }

  /** O mouse parou numa faixa: boa chance de ela ser clicada. */
  prepareHover(track: Track) {
    void api.prepare([$state.snapshot(track)], true).catch(() => {});
  }

  setTab(tab: Tab) {
    this.tab = tab;
    this.sidebarOpen = true;
  }

  toggleSidebar() {
    this.sidebarOpen = !this.sidebarOpen;
  }

  async search(query: string) {
    this.query = query;
    const seq = ++this.#searchSeq;
    if (!query.trim()) {
      this.results = [];
      this.searching = false;
      return;
    }
    this.searching = true;
    try {
      const results = await api.search(query);
      if (seq === this.#searchSeq) {
        this.results = results;
        this.prepareVisible(this.tracks);
        const all = interleave(results.map((r) => r.items));
        if (all.length) {
          this.lastSearch = { query, tracks: all.slice(0, MAX_LAST_SEARCH) };
          store(LAST_SEARCH_KEY, $state.snapshot(this.lastSearch));
        }
      }
    } catch (e) {
      if (seq === this.#searchSeq) this.notice = errorMessage(e);
    } finally {
      if (seq === this.#searchSeq) this.searching = false;
    }
  }

  async loadLibrary() {
    clearTimeout(this.#libraryRetry);
    this.playlists = await api.libraryPlaylists();
    this.#libraryAt = Date.now();
    // O Spotify pediu um tempo (429): a biblioteca tenta de novo sozinha quando o prazo passar.
    const wait = Math.max(0, ...this.playlists.map((r) => r.retryIn ?? 0));
    if (wait) this.#libraryRetry = setTimeout(() => void this.loadLibrary().catch(() => {}), (wait + 1) * 1000);
  }

  /**
   * A janela voltou a ter foco: o que pode ter mudado lá fora é lido de novo, sem piscar a tela
   * (a lista antiga fica até a nova chegar).
   */
  async refreshStale() {
    const now = Date.now();
    if (now - this.#libraryAt > STALE_AFTER_MS) void this.loadLibrary().catch(() => {});
    const opened = this.opened;
    if (opened?.kind !== "playlist" || !opened.source || opened.loading || now - this.#openedAt < STALE_AFTER_MS) return;
    const seq = this.#playlistSeq;
    this.#openedAt = now;
    try {
      const tracks = await api.playlistTracks(opened.source, opened.id);
      if (seq === this.#playlistSeq && this.opened) this.opened = { ...this.opened, tracks };
    } catch {
      // Sem rede agora: fica a lista que já estava na tela.
    }
  }

  async loadAccounts() {
    this.accounts = await api.accounts();
  }

  async connect(source: Source) {
    this.busySource = source;
    this.notice = null;
    try {
      await api.connectAccount(source);
      await Promise.all([this.loadAccounts(), this.loadLibrary()]);
    } catch (e) {
      this.notice = errorMessage(e);
    } finally {
      this.busySource = null;
    }
  }

  async disconnect(source: Source) {
    await api.disconnectAccount(source);
    await Promise.all([this.loadAccounts(), this.loadLibrary()]);
  }

  async play(tracks: Track[], start: number, contextName: string | null = null) {
    this.#applyPlayer(await api.play($state.snapshot(tracks), start, contextName));
  }

  async enqueue(track: Track) {
    this.#applyPlayer(await api.enqueue($state.snapshot(track)));
    this.notify(`"${track.title}" entrou na sua fila`);
  }

  async playNext(track: Track) {
    this.#applyPlayer(await api.playNext($state.snapshot(track)));
    this.notify(`"${track.title}" toca a seguir`);
  }

  async removeFromQueue(slot: QueueSlot) {
    this.#applyPlayer(await api.removeFromQueue(slot));
  }

  async jumpTo(slot: QueueSlot) {
    this.#applyPlayer(await api.jumpTo(slot));
  }

  /** Curte/descurte no GlobalMusic e no serviço de origem da faixa. */
  async toggleFavorite(track: Track) {
    const liked = !this.isFavorite(track);
    try {
      const result = await api.setFavorite($state.snapshot(track), liked);
      this.favorites = result.favorites;
      if (this.opened?.kind === "favorites") this.opened = { ...this.opened, tracks: result.favorites };
      const where = `no GlobalMusic e no ${SOURCE_LABEL[track.source]}`;
      this.notify(
        result.remoteError
          ? `${liked ? "Curtida" : "Removida"} no GlobalMusic. ${result.remoteError}`
          : liked
            ? `Curtida ${where}`
            : `Removida das curtidas ${where}`,
      );
    } catch (e) {
      this.notify(errorMessage(e));
    }
  }

  /** Cria uma playlist no GlobalMusic e já a abre. */
  async createPlaylist(name: string, tracks: Track[]) {
    try {
      this.locals = await api.createLocalPlaylist(name, $state.snapshot(tracks));
      this.creating = null;
      this.setTab("library");
      this.openLocal(this.locals[0]);
      this.notify(`Playlist "${this.locals[0].name}" criada`);
      return true;
    } catch (e) {
      this.notify(errorMessage(e));
      return false;
    }
  }

  async addToPlaylist(playlist: LocalPlaylist, tracks: Track[]) {
    try {
      this.locals = await api.addToLocalPlaylist(playlist.id, $state.snapshot(tracks));
      this.#refreshOpenedLocal();
      this.notify(tracks.length === 1 ? `"${tracks[0].title}" entrou em "${playlist.name}"` : `Músicas adicionadas a "${playlist.name}"`);
    } catch (e) {
      this.notify(errorMessage(e));
    }
  }

  async removeFromPlaylist(id: string, index: number) {
    this.locals = await api.removeFromLocalPlaylist(id, index);
    this.#refreshOpenedLocal();
  }

  async deletePlaylist(playlist: LocalPlaylist) {
    this.locals = await api.deleteLocalPlaylist(playlist.id);
    if (this.opened?.kind === "local" && this.opened.id === playlist.id) this.closePlaylist();
    this.notify(`Playlist "${playlist.name}" apagada`);
  }

  /** Nome e/ou foto de uma playlist do GlobalMusic. */
  async editPlaylist(playlist: LocalPlaylist, changes: { name?: string; cover?: string; removeCover?: boolean }) {
    try {
      this.locals = await api.editLocalPlaylist(playlist.id, changes);
      this.#refreshOpenedLocal();
      return true;
    } catch (e) {
      this.notify(errorMessage(e));
      return false;
    }
  }

  /** Compartilha uma playlist do GlobalMusic: o servidor guarda uma cópia e o link vai para a área de transferência. */
  async sharePlaylist(playlist: LocalPlaylist) {
    try {
      const url = await api.shareLocalPlaylist(playlist.id);
      await this.copyLink(url, `Link de "${playlist.name}" copiado. Quem receber cola em Nova playlist → Importar por link`);
    } catch (e) {
      const message = errorMessage(e);
      this.notify(message.includes("entre na sua conta") ? "Entre na sua conta GlobalMusic (Perfil) para compartilhar playlists" : message);
    }
  }

  async copyLink(url: string, message = "Link copiado") {
    try {
      await navigator.clipboard.writeText(url);
      this.notify(message);
    } catch {
      this.notify(url);
    }
  }

  openPlaylistMenu(target: PlaylistTarget, event: MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    this.menu = null;
    this.playlistMenu = { target, x: event.clientX, y: event.clientY };
  }

  /** Toca uma playlist de serviço sem abrir (carrega as faixas e começa da primeira). */
  async playService(playlist: Playlist) {
    try {
      const tracks = await api.playlistTracks(playlist.source, playlist.id);
      if (tracks.length) await this.play(tracks, 0, playlist.name);
      else this.notify(`"${playlist.name}" está vazia`);
    } catch (e) {
      this.notify(errorMessage(e));
    }
  }

  openLocal(playlist: LocalPlaylist) {
    this.#playlistSeq++;
    this.opened = {
      kind: "local",
      id: playlist.id,
      title: playlist.name,
      source: null,
      artworkUrl: null,
      tracks: playlist.tracks,
      loading: false,
      error: null,
    };
    this.prepareVisible(playlist.tracks);
  }

  #refreshOpenedLocal() {
    const opened = this.opened;
    if (opened?.kind !== "local") return;
    const fresh = this.locals.find((p) => p.id === opened.id);
    if (fresh) this.opened = { ...opened, title: fresh.name, tracks: fresh.tracks };
  }

  #rememberPlayed(track: Track) {
    if (this.recent[0] && sameTrack(this.recent[0], track)) return;
    this.recent = [track, ...this.recent.filter((t) => !sameTrack(t, track))].slice(0, MAX_RECENT);
    store(RECENT_KEY, $state.snapshot(this.recent));
  }

  /** Abre a playlist na biblioteca (lista as faixas); tocar é uma escolha separada. */
  async openPlaylist(playlist: Playlist) {
    const seq = ++this.#playlistSeq;
    this.opened = {
      kind: "playlist",
      id: playlist.id,
      title: playlist.name,
      source: playlist.source,
      artworkUrl: playlist.artwork?.thumb ?? null,
      link: playlistUrl(playlist),
      tracks: [],
      loading: true,
      error: null,
    };
    try {
      const tracks = await api.playlistTracks(playlist.source, playlist.id);
      if (seq === this.#playlistSeq && this.opened) {
        this.#openedAt = Date.now();
        this.opened = { ...this.opened, tracks, loading: false };
        this.prepareVisible(tracks);
      }
    } catch (e) {
      if (seq === this.#playlistSeq && this.opened) this.opened = { ...this.opened, loading: false, error: errorMessage(e) };
    }
  }

  openFavorites() {
    this.#playlistSeq++;
    this.opened = {
      kind: "favorites",
      id: "globalmusic-favorites",
      title: "Curtidas no GlobalMusic",
      source: null,
      artworkUrl: null,
      tracks: this.favorites,
      loading: false,
      error: null,
    };
    this.prepareVisible(this.favorites);
  }

  closePlaylist() {
    this.#playlistSeq++;
    this.opened = null;
  }

  openMenu(track: Track, event: MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    this.playlistMenu = null;
    this.menu = { track, x: event.clientX, y: event.clientY };
  }

  toggleLyrics() {
    this.showLyrics = !this.showLyrics;
    if (this.showLyrics) void this.loadLyrics();
  }

  /** Carrega a letra da faixa atual, se ainda não estiver carregada (chamada também ao trocar de música). */
  async loadLyrics() {
    const track = this.player.current;
    if (!this.showLyrics || !track) return;
    const key = trackKey(track);
    if (this.lyrics?.key === key) return;
    this.lyrics = { key, loading: true, data: null, error: null };
    try {
      const data = await api.lyrics($state.snapshot(track));
      if (this.lyrics?.key === key) this.lyrics = { key, loading: false, data, error: null };
    } catch (e) {
      if (this.lyrics?.key === key) this.lyrics = { key, loading: false, data: null, error: errorMessage(e) };
    }
  }

  async openVersions(track: Track) {
    this.versions = { track, loading: true, current: null, options: [], error: null };
    try {
      const found = await api.trackAlternatives($state.snapshot(track));
      if (this.versions?.track === track) this.versions = { ...this.versions, loading: false, ...found };
    } catch (e) {
      if (this.versions?.track === track) this.versions = { ...this.versions, loading: false, error: errorMessage(e) };
    }
  }

  async chooseVersion(option: Track) {
    const picker = this.versions;
    if (!picker) return;
    this.versions = null;
    try {
      await api.chooseTrackVersion($state.snapshot(picker.track), option.id);
      this.notify(`"${picker.track.title}" vai tocar com a versão escolhida`);
    } catch (e) {
      this.notify(errorMessage(e));
    }
  }

  /** Arrastando a barra: só a UI se move; o backend (e o Discord) recebem no fim (`commitSeek`). */
  previewSeek(positionMs: number) {
    this.positionMs = positionMs;
  }

  async commitSeek(positionMs: number) {
    this.#applyPlayer(await api.seek(positionMs));
  }

  // A posição oficial vem do backend; entre um snapshot e outro a UI avança sozinha (#tick).
  #applyPlayer(next: PlayerSnapshot) {
    // Resposta de um comando que chegou depois do evento de uma mudança mais nova: já está velha.
    if (next.version < this.player.version) return;
    this.positionMs = next.positionMs;
    this.player = next;
    if (next.current && next.playing && !next.loading) this.#rememberPlayed(next.current);
    void this.loadLyrics();
  }

  #tick() {
    const current = this.player.current;
    // Carregando = ainda sem som: o relógio fica parado até o backend avisar que começou.
    if (!this.player.playing || this.player.loading || !current) return;
    this.positionMs = Math.min(this.positionMs + TICK_MS, current.durationMs);
    // O fim da faixa é decidido pelo backend (o áudio de verdade acabou), não por este relógio.
  }
}

export const app = new AppStore();

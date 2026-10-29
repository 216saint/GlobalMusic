// Espelho de `src-tauri/src/models.rs` e `player/mod.rs`.

export type Source = "spotify" | "youtube_music" | "soundcloud";

export const SOURCES: Source[] = ["spotify", "youtube_music", "soundcloud"];

export const SOURCE_LABEL: Record<Source, string> = {
  spotify: "Spotify",
  youtube_music: "YouTube Music",
  soundcloud: "SoundCloud",
};

/** `thumb` (~160px) para listas e fundo; `full` (~544px) só no "tocando agora". */
export interface Artwork {
  thumb: string;
  full: string;
}

export interface Track {
  id: string;
  source: Source;
  title: string;
  artists: string[];
  album: string | null;
  durationMs: number;
  artwork: Artwork | null;
  isrc: string | null;
}

export interface Playlist {
  id: string;
  source: Source;
  name: string;
  owner: string | null;
  trackCount: number | null;
  artwork: Artwork | null;
  /** Link público quando não dá para montar pelo id (SoundCloud). */
  url?: string | null;
}

/** Playlist criada no GlobalMusic: faixas de qualquer fonte, sem precisar de conta. */
export interface LocalPlaylist {
  id: string;
  name: string;
  tracks: Track[];
  /** Foto escolhida pela pessoa (data URL JPEG); sem ela, mosaico das capas. */
  cover?: string | null;
}

/** Resposta de "importar por link": as listas atualizadas e o nome do que entrou. */
export interface ImportOutcome {
  name: string;
  imported: Playlist[];
  locals: LocalPlaylist[];
}

export interface SourceResults<T> {
  source: Source;
  items: T[];
  error: string | null;
  /** A fonte pediu para esperar (429): em quantos segundos vale tentar de novo. */
  retryIn?: number;
}

export interface AccountStatus {
  source: Source;
  connected: boolean;
  displayName: string | null;
  avatarUrl: string | null;
  premium: boolean | null;
  /** Algo que a pessoa precisa fazer nesta conta (ex.: reconectar). */
  notice: string | null;
}

export type PlaybackRoute = { kind: "spotify_native" } | { kind: "youtube_fallback" } | { kind: "direct_stream" };

export interface FavoriteResult {
  favorites: Track[];
  remoteError: string | null;
}

/** Link público da faixa no serviço de origem (quando dá para montar só com o ID). */
/** Link público de uma playlist de serviço (as coleções de curtidas são privadas: sem link). */
export function playlistUrl(playlist: Playlist): string | null {
  if (isLikedCollection(playlist)) return null;
  if (playlist.source === "spotify") return `https://open.spotify.com/playlist/${playlist.id}`;
  if (playlist.source === "youtube_music") return `https://music.youtube.com/playlist?list=${playlist.id}`;
  return playlist.url ?? null;
}

export function trackUrl(track: Track): string | null {
  if (track.source === "spotify") return `https://open.spotify.com/track/${track.id}`;
  if (track.source === "youtube_music") return `https://music.youtube.com/watch?v=${track.id}`;
  return null;
}

/** Espelho de `src-tauri/src/updates.rs`. */
export interface UpdateInfo {
  version: string;
  currentVersion: string;
  notes: string | null;
}

export interface UpdateProgress {
  downloaded: number;
  total: number | null;
}

export interface PlayerSnapshot {
  current: Track | null;
  route: PlaybackRoute | null;
  /** A lista de onde a pessoa deu play (playlist, busca…) e onde ela está nessa lista. */
  context: Track[];
  contextIndex: number | null;
  contextName: string | null;
  /** A sua fila: o que foi posto à mão. Toca antes do resto do contexto. */
  upNext: Track[];
  /** Intenção de tocar (verdadeiro também enquanto carrega). */
  playing: boolean;
  /** Resolvendo o áudio: ainda sem som. */
  loading: boolean;
  /** Posição no instante do snapshot; a UI avança localmente a partir daqui. */
  positionMs: number;
  volume: number;
  /** Ordem dos snapshots: um mais velho que o atual é descartado (chegou atrasado). */
  version: number;
}

// Letras (LRCLIB) e versões no YouTube: espelho de `lyrics.rs` e `commands.rs`.

export interface LyricLine {
  timeMs: number;
  text: string;
}

export interface Lyrics {
  /** Versos com tempo (vazio se só houver a letra corrida). */
  synced: LyricLine[];
  plain: string | null;
  instrumental: boolean;
}

export interface Alternatives {
  /** O vídeo que toca hoje no lugar da faixa. */
  current: string | null;
  options: Track[];
}

/** Ids da lista de curtidas de cada serviço (ver os `LIKED_ID`/`LIKES_ID` dos providers no Rust). */
const LIKED_IDS: Record<Source, string> = { spotify: "liked", youtube_music: "LM", soundcloud: "likes" };

/** É a lista de curtidas do serviço (ganha capa própria)? */
export const isLikedCollection = (p: { source: Source | null; id: string }) =>
  p.source !== null && LIKED_IDS[p.source] === p.id;

/** Uma faixa que ainda vai tocar: na sua fila ou no resto do contexto (`player/queue.rs`). */
export type QueueSlot = { list: "queued"; index: number } | { list: "context"; index: number };

// Espelho de `src-tauri/src/social.rs` (servidor social do GlobalMusic).

export type LoginProvider = "discord" | "spotify";

/** A própria conta GlobalMusic. */
export interface Me {
  id: number;
  username: string;
  displayName: string;
  avatarUrl: string | null;
  discordLinked: boolean;
  spotifyLinked: boolean;
  /** Nome e foto de cada login vinculado, para escolher qual usar no perfil. */
  linkedProfiles: LinkedProfile[];
}

export interface LinkedProfile {
  provider: LoginProvider;
  name: string;
  avatarUrl: string | null;
}

/** O que os outros veem de alguém. */
export interface PublicUser {
  username: string;
  displayName: string;
  avatarUrl: string | null;
}

export interface FriendList {
  friends: PublicUser[];
  /** Pedidos que chegaram (esperando você aceitar). */
  incoming: PublicUser[];
  /** Pedidos que você mandou (esperando a outra pessoa). */
  outgoing: PublicUser[];
}

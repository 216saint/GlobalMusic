// Backend falso para desenhar a UI no navegador (`npm run dev`) sem o Rust.
// Nunca é carregado dentro do app Tauri.

import type { Backend } from "./api";
import type {
  AccountStatus,
  FriendList,
  Me,
  PlayerSnapshot,
  Playlist,
  PublicUser,
  Source,
  SourceResults,
  Track,
} from "./types";
import { SOURCES } from "./types";

const t = (id: string, source: Source, title: string, artist: string, secs: number): Track => ({
  id,
  source,
  title,
  artists: [artist],
  album: null,
  durationMs: secs * 1000,
  artwork: null,
  isrc: null,
});

const TRACKS: Track[] = [
  t("s1", "spotify", "Morning Dew", "Xavier Wulf", 152),
  t("y1", "youtube_music", "worth it", "xaviersobased", 92),
  t("c1", "soundcloud", "love hate", "xaviersobased", 118),
  t("s2", "spotify", "Hollow Be Thy Squad", "Xavier Wulf", 141),
  t("y2", "youtube_music", "First Light", "Xavier Wulf", 133),
  t("c2", "soundcloud", "Smoke (demo)", "Xavier Wulf", 104),
  t("s3", "spotify", "Rally Gang", "Xavier Wulf", 167),
  t("y3", "youtube_music", "Coração de Gelo", "WIU", 181),
];

const PLAYLISTS: Playlist[] = [
  {
    id: "p1",
    source: "spotify",
    name: "Músicas Curtidas",
    owner: "você",
    trackCount: 412,
    artwork: null,
  },
  {
    id: "p2",
    source: "youtube_music",
    name: "Independentes BR",
    owner: "você",
    trackCount: 58,
    artwork: null,
  },
  {
    id: "p3",
    source: "soundcloud",
    name: "Underground",
    owner: "você",
    trackCount: 33,
    artwork: null,
  },
  {
    id: "p4",
    source: "spotify",
    name: "Foco",
    owner: "você",
    trackCount: 120,
    artwork: null,
  },
  {
    id: "p5",
    source: "youtube_music",
    name: "Curtidas",
    owner: "você",
    trackCount: 201,
    artwork: null,
  },
  {
    id: "p6",
    source: "soundcloud",
    name: "Achados da semana",
    owner: "você",
    trackCount: 17,
    artwork: null,
  },
];

const accounts: AccountStatus[] = SOURCES.map((source) => ({
  source,
  connected: source !== "soundcloud",
  displayName: source === "soundcloud" ? null : "Você",
  avatarUrl: null,
  premium: source === "spotify" ? false : null,
  notice: null,
}));

let player: PlayerSnapshot = {
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
};
const listeners = new Set<(s: PlayerSnapshot) => void>();
/** A faixa atual veio da sua fila (mesma regra do `player/queue.rs`). */
let nowQueued = false;

// Mesmo relógio do `player/clock.rs`: base + tempo corrido desde que voltou a tocar.
let base = 0;
let since: number | null = null;
const position = () => base + (since === null ? 0 : Date.now() - since);
function restart(playing: boolean, at = 0) {
  base = at;
  since = playing ? Date.now() : null;
}

function route(track: Track | null): PlayerSnapshot["route"] {
  if (!track) return null;
  if (track.source !== "spotify") return { kind: "direct_stream" };
  return accounts[0].premium ? { kind: "spotify_native" } : { kind: "youtube_fallback" };
}

function set(next: Partial<PlayerSnapshot>): PlayerSnapshot {
  player = { ...player, ...next };
  player.route = route(player.current);
  player.positionMs = Math.min(position(), player.current?.durationMs ?? 0);
  listeners.forEach((cb) => cb(player));
  return player;
}

/** "Adicionar à fila" / "Tocar a seguir": entra na sua fila; com nada tocando, já começa. */
function addToQueue(track: Track, first: boolean): PlayerSnapshot {
  const upNext = first ? [track, ...player.upNext] : [...player.upNext, track];
  if (player.current) return set({ upNext });
  restart(true);
  nowQueued = true;
  const [now, ...rest] = upNext;
  return set({ current: now, upNext: rest, playing: true });
}

const bySource = <T extends { source: Source }>(items: T[]): SourceResults<T>[] =>
  SOURCES.map((source) => ({
    source,
    items: items.filter((i) => i.source === source),
    error: null,
  }));

const commands: Record<string, (args: any) => unknown> = {
  search: ({ query }) => {
    const q = String(query).toLowerCase();
    const hits = TRACKS.filter((t) => `${t.title} ${t.artists.join(" ")}`.toLowerCase().includes(q));
    return bySource(hits.length ? hits : TRACKS);
  },
  library_playlists: () =>
    bySource(PLAYLISTS).map((r) =>
      accounts.find((a) => a.source === r.source)?.connected
        ? r
        : { ...r, items: [], error: `${r.source} não está conectado` },
    ),
  playlist_tracks: ({ source }) => TRACKS.filter((t) => t.source === source),
  accounts: () => accounts,
  connect_account: ({ source }) => {
    const account = accounts.find((a) => a.source === source)!;
    Object.assign(account, { connected: true, displayName: "Você" });
    return account;
  },
  disconnect_account: ({ source }) => {
    Object.assign(
      accounts.find((a) => a.source === source)!,
      { connected: false, displayName: null },
    );
  },
  // `?update` na URL simula uma atualização disponível, para desenhar o card no navegador.
  update_check: () =>
    location.search.includes("update")
      ? {
          version: "0.2.0",
          currentVersion: "0.1.0",
          notes: "Fila com arrastar e soltar.\nCorreção no login do SoundCloud.",
        }
      : null,
  update_install: () => new Promise(() => {}),
  spotify_custom_client_id: () => null,
  set_spotify_custom_client_id: () => undefined,
  player_state: () => player,
  player_play: ({ tracks, start, contextName }) => {
    restart(true);
    nowQueued = false;
    return set({ context: tracks, contextIndex: start, contextName, current: tracks[start] ?? null, playing: true });
  },
  player_toggle: () => {
    const playing = !player.playing && player.current !== null;
    restart(playing, position());
    return set({ playing });
  },
  player_next: () => {
    const [queued, ...upNext] = player.upNext;
    if (queued) {
      restart(true);
      nowQueued = true;
      return set({ current: queued, upNext, playing: true });
    }
    const next = (player.contextIndex ?? -1) + 1;
    if (next >= player.context.length) {
      restart(false);
      return set({ playing: false });
    }
    restart(true);
    nowQueued = false;
    return set({ contextIndex: next, current: player.context[next], playing: true });
  },
  player_previous: () => {
    if (player.contextIndex === null) return player;
    const target = nowQueued ? player.contextIndex : Math.max(0, player.contextIndex - 1);
    restart(player.playing);
    nowQueued = false;
    return set({ contextIndex: target, current: player.context[target] });
  },
  player_seek: ({ positionMs }) => {
    restart(player.playing, positionMs);
    return set({});
  },
  player_set_volume: ({ volume }) => set({ volume: Math.min(1, Math.max(0, volume)) }),
  player_enqueue: ({ track }) => addToQueue(track, false),
  player_play_next: ({ track }) => addToQueue(track, true),
  player_remove: ({ slot }) => {
    if (slot.list === "queued") return set({ upNext: player.upNext.filter((_, i) => i !== slot.index) });
    if (!nowQueued && slot.index === player.contextIndex) return player;
    const at = player.contextIndex;
    return set({
      context: player.context.filter((_, i) => i !== slot.index),
      contextIndex: at !== null && at > slot.index ? at - 1 : at,
    });
  },
  player_prepare: () => undefined,
  lyrics: () => ({
    synced: [
      { timeMs: 0, text: "Primeiro verso da demonstração" },
      { timeMs: 4000, text: "O verso atual fica destacado" },
      { timeMs: 8000, text: "e a letra acompanha a música" },
      { timeMs: 12000, text: "Clique num verso para pular até ele" },
    ],
    plain: null,
    instrumental: false,
  }),
  track_alternatives: ({ track }) => ({
    current: "y1",
    options: TRACKS.filter((t) => t.source === "youtube_music").map((t) => ({ ...t, title: `${track.title} (${t.title})` })),
  }),
  choose_track_version: () => undefined,
  player_jump: ({ slot }) => {
    restart(true);
    if (slot.list === "queued") {
      nowQueued = true;
      return set({ current: player.upNext[slot.index], upNext: player.upNext.slice(slot.index + 1), playing: true });
    }
    nowQueued = false;
    return set({ contextIndex: slot.index, current: player.context[slot.index], playing: true });
  },
  favorites_list: () => favorites,
  set_favorite: ({ track, liked }) => {
    favorites = favorites.filter((f) => !(f.source === track.source && f.id === track.id));
    if (liked) favorites = [track, ...favorites];
    return { favorites, remoteError: null };
  },
};

let favorites: Track[] = [];

// Conta GlobalMusic de mentira: começa deslogada; `?logado` na URL já entra.
let me: Me | null = location.search.includes("logado")
  ? {
      id: 1,
      username: "voce",
      displayName: "Você",
      avatarUrl: null,
      discordLinked: true,
      spotifyLinked: false,
    }
  : null;
const PEOPLE: PublicUser[] = [
  { username: "ana.beats", displayName: "Ana", avatarUrl: null },
  { username: "caio_sc", displayName: "Caio", avatarUrl: null },
  { username: "duda", displayName: "Duda", avatarUrl: null },
];
const friends: FriendList = {
  friends: [PEOPLE[0]],
  incoming: [PEOPLE[1]],
  outgoing: [],
};
const move = (username: string, to: PublicUser[] | null) => {
  const user = PEOPLE.find((p) => p.username === username)!;
  for (const list of [friends.friends, friends.incoming, friends.outgoing]) {
    const i = list.findIndex((u) => u.username === username);
    if (i >= 0) list.splice(i, 1);
  }
  to?.push(user);
  return structuredClone(friends);
};

Object.assign(commands, {
  social_me: () => me,
  social_login: ({ provider }: { provider: string }) =>
    (me = {
      id: 1,
      username: "voce",
      displayName: "Você",
      avatarUrl: null,
      discordLinked: provider === "discord",
      spotifyLinked: provider === "spotify",
    }),
  social_link: ({ provider }: { provider: string }) => (me = { ...me!, [`${provider}Linked`]: true }),
  social_logout: () => void (me = null),
  social_update_profile: (changes: Partial<Me>) => (me = { ...me!, ...changes }),
  social_upload_avatar: (bytes: Uint8Array) =>
    (me = {
      ...me!,
      avatarUrl: URL.createObjectURL(new Blob([bytes as BlobPart], { type: "image/jpeg" })),
    }),
  social_search: ({ query }: { query: string }) =>
    PEOPLE.filter((p) => p.username.startsWith(query.replace(/^@/, "").toLowerCase())),
  social_friends: () => structuredClone(friends),
  social_add_friend: ({ username }: { username: string }) =>
    move(username, friends.incoming.some((u) => u.username === username) ? friends.friends : friends.outgoing),
  social_remove_friend: ({ username }: { username: string }) => move(username, null),
});

export const demoBackend: Backend = {
  async call<T>(cmd: string, args: unknown = {}) {
    await new Promise((r) => setTimeout(r, 120));
    return commands[cmd](args) as T;
  },
  async onPlayer(cb) {
    listeners.add(cb);
    return () => listeners.delete(cb);
  },
  // A demo não toca áudio, então nunca falha nem pula trechos.
  async onPlayerError() {
    return () => {};
  },
  async onPlayerNotice() {
    return () => {};
  },
  async onUpdateProgress() {
    return () => {};
  },
};

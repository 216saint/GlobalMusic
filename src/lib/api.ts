// Wrappers tipados dos comandos de `src-tauri/src/commands.rs`.
// Fora do Tauri (ex.: `npm run dev` aberto no navegador) cai num backend de
// demonstração, carregado sob demanda para não pesar no app real.

import { invoke, isTauri, type InvokeArgs } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AccountStatus,
  Alternatives,
  FavoriteResult,
  FriendList,
  LocalPlaylist,
  LoginProvider,
  Lyrics,
  Me,
  PlayerSnapshot,
  Playlist,
  PublicUser,
  QueueSlot,
  Source,
  SourceResults,
  Track,
  UpdateInfo,
  UpdateProgress,
} from "./types";

export interface Backend {
  /** `args` também pode ser bytes crus (`Uint8Array`), sem passar por JSON. */
  call<T>(cmd: string, args?: InvokeArgs): Promise<T>;
  onPlayer(cb: (snapshot: PlayerSnapshot) => void): Promise<() => void>;
  /** Uma faixa não pôde tocar (o player já pulou para a próxima). */
  onPlayerError(cb: (message: string) => void): Promise<() => void>;
  /** Aviso curto do player (ex.: "Pulei um trecho sem música"). */
  onPlayerNotice(cb: (message: string) => void): Promise<() => void>;
  onUpdateProgress(cb: (progress: UpdateProgress) => void): Promise<() => void>;
}

const tauriBackend: Backend = {
  call: (cmd, args) => invoke(cmd, args),
  onPlayer: (cb) => listen<PlayerSnapshot>("player://state", (e) => cb(e.payload)),
  onPlayerError: (cb) => listen<string>("player://error", (e) => cb(e.payload)),
  onPlayerNotice: (cb) => listen<string>("player://notice", (e) => cb(e.payload)),
  onUpdateProgress: (cb) => listen<UpdateProgress>("update://progress", (e) => cb(e.payload)),
};

let backend: Promise<Backend> | undefined;
function getBackend(): Promise<Backend> {
  backend ??= isTauri() ? Promise.resolve(tauriBackend) : import("./demo").then((m) => m.demoBackend);
  return backend;
}

const call = async <T>(cmd: string, args?: InvokeArgs) => (await getBackend()).call<T>(cmd, args);

export const api = {
  search: (query: string, limit = 10) => call<SourceResults<Track>[]>("search", { query, limit }),
  libraryPlaylists: () => call<SourceResults<Playlist>[]>("library_playlists"),
  importedPlaylists: () => call<Playlist[]>("imported_playlists"),
  /** Importa por link (Spotify, YouTube Music ou SoundCloud); devolve as importadas. */
  importPlaylist: (link: string) => call<Playlist[]>("import_playlist", { link }),
  removeImportedPlaylist: (source: Source, id: string) => call<Playlist[]>("remove_imported_playlist", { source, id }),
  localPlaylists: () => call<LocalPlaylist[]>("local_playlists"),
  createLocalPlaylist: (name: string, tracks: Track[]) => call<LocalPlaylist[]>("create_local_playlist", { name, tracks }),
  addToLocalPlaylist: (id: string, tracks: Track[]) => call<LocalPlaylist[]>("add_to_local_playlist", { id, tracks }),
  removeFromLocalPlaylist: (id: string, index: number) => call<LocalPlaylist[]>("remove_from_local_playlist", { id, index }),
  deleteLocalPlaylist: (id: string) => call<LocalPlaylist[]>("delete_local_playlist", { id }),
  playlistTracks: (source: Source, playlistId: string) => call<Track[]>("playlist_tracks", { source, playlistId }),

  accounts: () => call<AccountStatus[]>("accounts"),
  connectAccount: (source: Source) => call<AccountStatus>("connect_account", { source }),
  disconnectAccount: (source: Source) => call<void>("disconnect_account", { source }),
  spotifyCustomClientId: () => call<string | null>("spotify_custom_client_id"),
  setSpotifyCustomClientId: (clientId: string | null) => call<void>("set_spotify_custom_client_id", { clientId }),

  playerState: () => call<PlayerSnapshot>("player_state"),
  /** `contextName`: o nome da lista (ex.: a playlist), para a fila dizer "A seguir de …". */
  play: (tracks: Track[], start: number, contextName: string | null = null) =>
    call<PlayerSnapshot>("player_play", { tracks, start, contextName }),
  toggle: () => call<PlayerSnapshot>("player_toggle"),
  next: () => call<PlayerSnapshot>("player_next"),
  previous: () => call<PlayerSnapshot>("player_previous"),
  seek: (positionMs: number) => call<PlayerSnapshot>("player_seek", { positionMs: Math.round(positionMs) }),
  enqueue: (track: Track) => call<PlayerSnapshot>("player_enqueue", { track }),
  playNext: (track: Track) => call<PlayerSnapshot>("player_play_next", { track }),
  removeFromQueue: (slot: QueueSlot) => call<PlayerSnapshot>("player_remove", { slot }),
  jumpTo: (slot: QueueSlot) => call<PlayerSnapshot>("player_jump", { slot }),
  /** Resolve o áudio antes do clique: o topo de uma lista na tela, ou a faixa sob o mouse (`hover`). */
  prepare: (tracks: Track[], hover = false) => call<void>("player_prepare", { tracks, hover }),

  favorites: () => call<Track[]>("favorites_list"),
  /** Curte no GlobalMusic e no serviço de origem; `remoteError` se o serviço recusou. */
  setFavorite: (track: Track, liked: boolean) => call<FavoriteResult>("set_favorite", { track, liked }),
  setVolume: (volume: number) => call<PlayerSnapshot>("player_set_volume", { volume }),

  onPlayerState: async (cb: (snapshot: PlayerSnapshot) => void) => (await getBackend()).onPlayer(cb),
  onPlayerError: async (cb: (message: string) => void) => (await getBackend()).onPlayerError(cb),
  onPlayerNotice: async (cb: (message: string) => void) => (await getBackend()).onPlayerNotice(cb),

  /** Letra pelo LRCLIB; `null` se não houver. */
  lyrics: (track: Track) => call<Lyrics | null>("lyrics", { track }),
  /** Versões no YouTube de uma faixa do Spotify (para corrigir uma equivalência errada). */
  trackAlternatives: (track: Track) => call<Alternatives>("track_alternatives", { track }),
  chooseTrackVersion: (track: Track, videoId: string) => call<void>("choose_track_version", { track, videoId }),

  checkUpdate: () => call<UpdateInfo | null>("update_check"),
  /** Baixa, confere a assinatura e instala; o app reinicia sozinho no fim. */
  installUpdate: () => call<void>("update_install"),
  onUpdateProgress: async (cb: (progress: UpdateProgress) => void) => (await getBackend()).onUpdateProgress(cb),

  // Conta GlobalMusic, perfil e amigos (servidor social).
  socialMe: () => call<Me | null>("social_me"),
  socialLogin: (provider: LoginProvider) => call<Me>("social_login", { provider }),
  socialLink: (provider: LoginProvider) => call<Me>("social_link", { provider }),
  socialLogout: () => call<void>("social_logout"),
  updateProfile: (changes: { displayName?: string; username?: string }) => call<Me>("social_update_profile", changes),
  /** Usa o nome e a foto do Discord ou do Spotify no perfil. */
  useLinkedProfile: (provider: LoginProvider) => call<Me>("social_use_profile", { provider }),
  /** JPEG já reduzido; vai como bytes crus pelo IPC. */
  uploadAvatar: (jpeg: Uint8Array) => call<Me>("social_upload_avatar", jpeg),
  searchUsers: (query: string) => call<PublicUser[]>("social_search", { query }),
  friends: () => call<FriendList>("social_friends"),
  addFriend: (username: string) => call<FriendList>("social_add_friend", { username }),
  removeFriend: (username: string) => call<FriendList>("social_remove_friend", { username }),
};

export const errorMessage = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : String(e));

// Conta GlobalMusic, perfil e amigos (Svelte 5 runes). Fala com o servidor social pelo Rust.

import { api, errorMessage } from "./api";
import { squareJpegBytes } from "./image";
import type { FriendList, LoginProvider, Me, PublicUser } from "./types";

const EMPTY_FRIENDS: FriendList = { friends: [], incoming: [], outgoing: [] };
/** Lado da foto enviada. O servidor reduz para 256 px; mandar maior que isso só gasta rede. */
const AVATAR_PX = 512;
const FRIENDS_REFRESH_MS = 60_000;

class SocialStore {
  me = $state<Me | null>(null);
  /** Já perguntou ao servidor se há sessão salva (evita piscar o "Entrar"). */
  ready = $state(false);
  friends = $state<FriendList>(EMPTY_FRIENDS);
  /** O que está em andamento, para desabilitar o botão certo. */
  busy = $state<LoginProvider | "avatar" | "profile" | null>(null);
  error = $state<string | null>(null);

  results = $state<PublicUser[]>([]);
  searching = $state(false);

  #searchSeq = 0;

  get pendingCount() {
    return this.friends.incoming.length;
  }

  async init() {
    try {
      this.me = await api.socialMe();
      if (this.me) await this.loadFriends();
    } catch {
      // Sem internet ou servidor fora do ar: o resto do app funciona igual.
    } finally {
      this.ready = true;
    }
    // Pedidos de amizade novos aparecem sem precisar reabrir o app.
    setInterval(() => {
      if (this.me) void this.loadFriends().catch(() => {});
    }, FRIENDS_REFRESH_MS);
  }

  async #run<T>(what: SocialStore["busy"], action: () => Promise<T>): Promise<T | undefined> {
    this.busy = what;
    this.error = null;
    try {
      return await action();
    } catch (e) {
      this.error = errorMessage(e);
      // A sessão expirou no meio do caminho: volta para a tela de entrar.
      this.me = await api.socialMe().catch(() => this.me);
      return undefined;
    } finally {
      this.busy = null;
    }
  }

  async login(provider: LoginProvider) {
    const me = await this.#run(provider, () => api.socialLogin(provider));
    if (me) {
      this.me = me;
      await this.loadFriends().catch(() => {});
    }
  }

  async link(provider: LoginProvider) {
    const me = await this.#run(provider, () => api.socialLink(provider));
    if (me) this.me = me;
  }

  async logout() {
    await api.socialLogout();
    this.me = null;
    this.friends = EMPTY_FRIENDS;
    this.results = [];
    this.error = null;
  }

  /** `true` se salvou (a UI fecha o modo de edição). */
  async saveProfile(changes: { displayName?: string; username?: string }) {
    const me = await this.#run("profile", () => api.updateProfile(changes));
    if (me) this.me = me;
    return me !== undefined;
  }

  async useLinkedProfile(provider: LoginProvider) {
    const me = await this.#run("profile", () => api.useLinkedProfile(provider));
    if (me) this.me = me;
  }

  async uploadAvatar(file: File) {
    const me = await this.#run("avatar", async () => api.uploadAvatar(await squareJpegBytes(file, AVATAR_PX)));
    if (me) this.me = me;
  }

  async loadFriends() {
    this.friends = await api.friends();
  }

  async search(query: string) {
    const seq = ++this.#searchSeq;
    if (query.trim().replace(/^@/, "").length < 2) {
      this.results = [];
      this.searching = false;
      return;
    }
    this.searching = true;
    try {
      const results = await api.searchUsers(query);
      if (seq === this.#searchSeq) this.results = results;
    } catch (e) {
      if (seq === this.#searchSeq) this.error = errorMessage(e);
    } finally {
      if (seq === this.#searchSeq) this.searching = false;
    }
  }

  /** Pedir amizade, ou aceitar um pedido recebido. */
  async addFriend(username: string) {
    const list = await this.#run(null, () => api.addFriend(username));
    if (list) this.friends = list;
  }

  /** Desfazer amizade, recusar um pedido ou cancelar um enviado. */
  async removeFriend(username: string) {
    const list = await this.#run(null, () => api.removeFriend(username));
    if (list) this.friends = list;
  }

  relation(username: string): "friend" | "incoming" | "outgoing" | null {
    const has = (list: PublicUser[]) => list.some((u) => u.username === username);
    if (has(this.friends.friends)) return "friend";
    if (has(this.friends.incoming)) return "incoming";
    if (has(this.friends.outgoing)) return "outgoing";
    return null;
  }
}

export const social = new SocialStore();

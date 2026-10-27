<!-- Conta GlobalMusic: entrar (Discord ou Spotify), foto, nome, @usuário e logins vinculados. -->
<script lang="ts">
  import Camera from "@lucide/svelte/icons/camera";
  import Pencil from "@lucide/svelte/icons/pencil";
  import { app } from "$lib/app.svelte";
  import { social } from "$lib/social.svelte";
  import Avatar from "./Avatar.svelte";

  let editing = $state(false);
  let nameInput = $state("");
  let usernameInput = $state("");
  let fileInput = $state<HTMLInputElement>();

  const spotifyConnected = $derived(app.accounts.some((a) => a.source === "spotify" && a.connected));

  function startEditing() {
    if (!social.me) return;
    nameInput = social.me.displayName;
    usernameInput = social.me.username;
    editing = true;
  }

  async function save(event: SubmitEvent) {
    event.preventDefault();
    const me = social.me;
    if (!me) return;
    const changes: { displayName?: string; username?: string } = {};
    if (nameInput.trim() !== me.displayName) changes.displayName = nameInput;
    if (usernameInput.trim().replace(/^@/, "") !== me.username) changes.username = usernameInput;
    if (!Object.keys(changes).length || (await social.saveProfile(changes))) editing = false;
  }

  function pickPhoto(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = ""; // escolher a mesma foto de novo dispara outra vez
    if (file) void social.uploadAvatar(file);
  }
</script>

<section class="card" aria-label="Conta GlobalMusic">
  {#if !social.ready}
    <p class="muted">Carregando sua conta…</p>
  {:else if social.me}
    {@const me = social.me}
    <div class="who">
      <button
        class="photo"
        title="Trocar foto"
        disabled={social.busy === "avatar"}
        onclick={() => fileInput?.click()}
      >
        <Avatar name={me.displayName} url={me.avatarUrl} size={64} />
        <span class="overlay" class:on={social.busy === "avatar"}>
          {#if social.busy === "avatar"}…{:else}<Camera size={18} />{/if}
        </span>
      </button>
      <input
        bind:this={fileInput}
        class="sr-only"
        type="file"
        accept="image/png,image/jpeg,image/webp,image/gif"
        onchange={pickPhoto}
      />

      {#if editing}
        <form class="edit" onsubmit={save}>
          <label>
            <span>Nome</span>
            <input bind:value={nameInput} maxlength="40" required />
          </label>
          <label>
            <span>@usuário</span>
            <input bind:value={usernameInput} maxlength="21" spellcheck="false" autocapitalize="off" required />
          </label>
          <div class="actions">
            <button class="btn" type="submit" disabled={social.busy === "profile"}>
              {social.busy === "profile" ? "Salvando…" : "Salvar"}
            </button>
            <button class="btn ghost" type="button" onclick={() => (editing = false)}>Cancelar</button>
          </div>
        </form>
      {:else}
        <div class="text">
          <strong>{me.displayName}</strong>
          <span class="handle">@{me.username}</span>
        </div>
        <button class="icon" title="Editar nome e @usuário" aria-label="Editar nome e @usuário" onclick={startEditing}>
          <Pencil size={15} />
        </button>
      {/if}
    </div>

    {#if me.linkedProfiles.length}
      <!-- Sincronizar o perfil: nome e foto vindos do Discord ou do Spotify. -->
      <div class="sync">
        <span class="muted">Usar o perfil do:</span>
        {#each me.linkedProfiles as profile (profile.provider)}
          {@const inUse = me.displayName === profile.name && me.avatarUrl === profile.avatarUrl}
          <button
            class="chip profile"
            class:in-use={inUse}
            disabled={inUse || social.busy !== null}
            title={inUse ? "Em uso" : `Usar "${profile.name}" e a foto do ${profile.provider === "discord" ? "Discord" : "Spotify"}`}
            onclick={() => social.useLinkedProfile(profile.provider)}
          >
            <Avatar name={profile.name} url={profile.avatarUrl} size={18} />
            {profile.provider === "discord" ? "Discord" : "Spotify"}
          </button>
        {/each}
      </div>
    {/if}

    <div class="logins">
      <span class="muted">Entrar com:</span>
      {#if me.discordLinked}
        <span class="chip">Discord</span>
      {:else}
        <button class="chip add" disabled={social.busy !== null} onclick={() => social.link("discord")}>
          {social.busy === "discord" ? "Aguardando o navegador…" : "+ Vincular Discord"}
        </button>
      {/if}
      {#if me.spotifyLinked}
        <span class="chip">Spotify</span>
      {:else}
        <button
          class="chip add"
          disabled={social.busy !== null || !spotifyConnected}
          title={spotifyConnected ? undefined : "Conecte o Spotify em Fontes de música primeiro"}
          onclick={() => social.link("spotify")}
        >
          + Vincular Spotify
        </button>
      {/if}
      <button class="btn ghost logout" onclick={() => social.logout()}>Sair</button>
    </div>
  {:else}
    <p class="lead">Entre para ter um perfil com foto e adicionar amigos.</p>
    <div class="actions">
      <button class="btn discord" disabled={social.busy !== null} onclick={() => social.login("discord")}>
        {social.busy === "discord" ? "Aguardando o navegador…" : "Entrar com Discord"}
      </button>
      <button
        class="btn spotify"
        disabled={social.busy !== null || !spotifyConnected}
        onclick={() => social.login("spotify")}
      >
        {social.busy === "spotify" ? "Entrando…" : "Entrar com Spotify"}
      </button>
    </div>
    {#if !spotifyConnected}
      <p class="muted small">Para entrar com o Spotify, conecte ele em Fontes de música, mais abaixo.</p>
    {/if}
  {/if}

  {#if social.error}
    <p class="error" role="alert">{social.error}</p>
  {/if}
</section>

<style>
  .card {
    padding: 18px;
    border-radius: 18px;
    background: var(--tint-1);
    border: 1px solid var(--edge);
  }

  .who {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .photo {
    position: relative;
    border-radius: 50%;
  }

  .overlay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.45);
    color: #fff;
    opacity: 0;
    transition: opacity 0.15s;
  }

  .photo:hover .overlay,
  .photo:focus-visible .overlay,
  .overlay.on {
    opacity: 1;
  }

  .text {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .text strong {
    font-family: var(--font-display);
    font-size: 18px;
    font-weight: 650;
  }

  .handle {
    color: var(--ink-2);
    font-size: 13px;
  }

  .icon {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 10px;
    color: var(--ink-2);
  }

  .icon:hover {
    color: var(--ink);
    background: var(--tint-2);
  }

  .edit {
    flex: 1;
    display: grid;
    gap: 8px;
  }

  .edit label {
    display: grid;
    gap: 3px;
    font-size: 12px;
    color: var(--ink-2);
  }

  .edit input {
    padding: 7px 12px;
    border-radius: 10px;
    border: 1px solid var(--edge);
    background: var(--tint-1);
    outline: none;
    color: var(--ink);
  }

  .edit input:focus {
    border-color: var(--violet);
  }

  .logins {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-top: 16px;
    font-size: 12.5px;
  }

  .chip {
    padding: 4px 10px;
    border-radius: 999px;
    border: 1px solid var(--edge);
    background: var(--tint-2);
  }

  .chip.add {
    background: none;
    color: var(--ink-2);
    border-style: dashed;
  }

  .chip.add:hover:not(:disabled) {
    color: var(--ink);
    border-color: var(--violet);
  }

  .chip.add:disabled {
    opacity: 0.55;
    cursor: default;
  }

  .logout {
    margin-left: auto;
  }

  .sync {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-top: 16px;
    font-size: 12.5px;
  }

  .chip.profile {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 10px 3px 4px;
    background: none;
    color: var(--ink-2);
  }

  .chip.profile:hover:not(:disabled) {
    color: var(--ink);
    border-color: var(--violet);
  }

  .chip.profile.in-use {
    background: var(--violet-dim);
    color: var(--ink);
    cursor: default;
  }

  .lead {
    margin: 0 0 14px;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .btn {
    padding: 7px 14px;
    border-radius: 10px;
    font-weight: 600;
    background: var(--violet);
    color: #fff;
  }

  .btn:hover:not(:disabled) {
    background: var(--violet-bright);
  }

  .btn:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .btn.discord {
    background: #5865f2;
  }

  .btn.discord:hover:not(:disabled) {
    background: #6d78f5;
  }

  .btn.spotify {
    background: var(--spotify);
    color: #06210f;
  }

  .btn.spotify:hover:not(:disabled) {
    background: #3be477;
  }

  .btn.ghost {
    background: none;
    color: var(--ink-2);
    border: 1px solid var(--edge);
  }

  .btn.ghost:hover {
    color: var(--ink);
    background: var(--tint-1);
  }

  .muted {
    margin: 0;
    color: var(--ink-2);
  }

  .small {
    margin-top: 10px;
    font-size: 12.5px;
  }

  .error {
    margin: 14px 0 0;
    padding: 9px 12px;
    border-radius: 12px;
    background: rgba(255, 90, 90, 0.12);
    border: 1px solid rgba(255, 90, 90, 0.3);
    color: #ffd5d5;
    font-size: 13px;
  }
</style>

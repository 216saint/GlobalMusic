<!-- Perfil: conta GlobalMusic e as três fontes de música (conectar/desconectar). Amigos têm aba própria. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import { api, errorMessage } from "$lib/api";
  import { app } from "$lib/app.svelte";
  import type { AccountStatus } from "$lib/types";
  import { SOURCE_LABEL } from "$lib/types";
  import SocialAccount from "./SocialAccount.svelte";
  import SourceIcon from "./SourceIcon.svelte";

  const REDIRECT_URI = "http://127.0.0.1:47823/login";

  // Client ID próprio do Spotify: cota de requisições só da pessoa (ver ADR 0003).
  let savedClientId = $state<string | null>(null);
  let clientIdInput = $state("");
  let clientIdMessage = $state<string | null>(null);

  let version = $state<string | null>(null);

  onMount(async () => {
    savedClientId = await api.spotifyCustomClientId();
    clientIdInput = savedClientId ?? "";
    version = await getVersion().catch(() => null); // fora do Tauri (demo) não há versão
  });

  async function saveClientId(value: string | null) {
    clientIdMessage = null;
    try {
      await api.setSpotifyCustomClientId(value);
      savedClientId = value?.trim() || null;
      clientIdInput = savedClientId ?? "";
      clientIdMessage = savedClientId
        ? "Salvo. Conecte o Spotify de novo para usar o seu Client ID."
        : "Voltou para o Client ID padrão. Conecte o Spotify de novo.";
      await app.loadAccounts();
    } catch (e) {
      clientIdMessage = errorMessage(e);
    }
  }

  function describe(a: AccountStatus) {
    if (!a.connected) return "Não conectado";
    const who = a.displayName ? `Conectado como ${a.displayName}` : "Conectado";
    if (a.source !== "spotify" || a.premium === null) return who;
    return a.premium ? `${who}, Premium` : `${who}, Free (o áudio vem do YouTube)`;
  }
</script>

<section class="profile" aria-label="Perfil">
  <button class="back" onclick={() => (app.panel = "now")}><ChevronLeft size={16} /> Voltar ao player</button>

  <h1>Perfil</h1>

  <!-- Duas colunas em janela larga; numa estreita, a segunda desce para baixo da primeira. -->
  <div class="columns">
    <div class="column">
      <h2>Sua conta</h2>
      <SocialAccount />
    </div>

    <div class="column">
      <h2>Fontes de música</h2>
      <p class="intro">Conecte as fontes para juntar a sua biblioteca e buscar em todas de uma vez.</p>

      <ul>
        {#each app.accounts as account (account.source)}
          <li class="account">
            <SourceIcon source={account.source} size={30} />
            <div class="text">
              <strong>{SOURCE_LABEL[account.source]}</strong>
              <span>{describe(account)}</span>
              {#if account.notice}
                <span class="account-notice">{account.notice}</span>
              {/if}
            </div>
            {#if account.connected}
              <button class="btn ghost" onclick={() => app.disconnect(account.source)}>Desconectar</button>
            {:else}
              <button class="btn" disabled={app.busySource !== null} onclick={() => app.connect(account.source)}>
                {app.busySource === account.source ? "Aguardando login…" : "Conectar"}
              </button>
            {/if}
          </li>
        {/each}
      </ul>

      {#if app.notice}
        <p class="notice" role="alert">{app.notice}</p>
      {/if}

      <details class="advanced" open={savedClientId !== null || app.notice?.includes("muitas requisições")}>
        <summary>Spotify recusando por excesso de requisições?</summary>
        <p>
          Por padrão, o GlobalMusic usa o mesmo Client ID de outros apps não oficiais do Spotify, e o Spotify divide o
          limite de requisições entre todos eles. Com um Client ID seu, o limite passa a ser só seu.
        </p>
        <ol>
          <li>Entre em <strong>developer.spotify.com/dashboard</strong> com a sua conta e clique em <em>Create app</em>.</li>
          <li>Em <em>Redirect URIs</em>, adicione <code>{REDIRECT_URI}</code>.</li>
          <li>Marque <em>Web API</em> e <em>Web Playback SDK</em> e salve.</li>
          <li>Copie o <em>Client ID</em> e cole abaixo.</li>
        </ol>
        <form
          class="client-id"
          onsubmit={(e) => {
            e.preventDefault();
            void saveClientId(clientIdInput);
          }}
        >
          <label class="sr-only" for="client-id">Client ID do Spotify</label>
          <input id="client-id" placeholder="32 caracteres, ex.: 1a2b3c…" bind:value={clientIdInput} spellcheck="false" />
          <button class="btn" type="submit">Salvar</button>
          {#if savedClientId}
            <button class="btn ghost" type="button" onclick={() => saveClientId(null)}>Usar o padrão</button>
          {/if}
        </form>
        {#if clientIdMessage}
          <p class="hint" role="status">{clientIdMessage}</p>
        {/if}
      </details>
    </div>
  </div>

  {#if version}
    <p class="version tabular">GlobalMusic {version}</p>
  {/if}
</section>

<style>
  .profile {
    padding: 28px 40px;
    overflow-y: auto;
  }

  .back {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--ink-2);
    font-size: 13px;
  }

  .back:hover {
    color: var(--ink);
  }

  h1 {
    margin: 28px 0 16px;
    font-family: var(--font-display);
    font-size: 28px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }

  .columns {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 380px), 1fr));
    gap: 32px 48px;
    align-items: start;
    max-width: 1500px;
  }

  .column {
    min-width: 0;
  }

  h2 {
    margin: 0 0 12px;
    font-family: var(--font-display);
    font-size: 20px;
    font-weight: 650;
  }

  .intro {
    margin: -6px 0 16px;
    color: var(--ink-2);
    max-width: 56ch;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 10px;
  }

  .account {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 16px;
    border-radius: 16px;
    background: var(--tint-1);
    border: 1px solid var(--edge);
  }

  .text {
    flex: 1;
    display: flex;
    flex-direction: column;
  }

  .text span {
    font-size: 12.5px;
    color: var(--ink-2);
  }

  .text .account-notice {
    color: var(--violet-bright);
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

  .btn.ghost {
    background: none;
    color: var(--ink-2);
    border: 1px solid var(--edge);
  }

  .btn.ghost:hover {
    color: var(--ink);
    background: var(--tint-1);
  }

  .notice {
    margin: 18px 0 0;
    padding: 10px 14px;
    border-radius: 12px;
    background: rgba(255, 90, 90, 0.12);
    border: 1px solid rgba(255, 90, 90, 0.3);
    color: #ffd5d5;
    font-size: 13px;
  }

  .advanced {
    margin-top: 28px;
    padding: 14px 16px;
    border-radius: 16px;
    border: 1px solid var(--edge);
    color: var(--ink-2);
    font-size: 13px;
  }

  .advanced summary {
    cursor: pointer;
    color: var(--ink);
    font-weight: 600;
  }

  .advanced ol {
    padding-left: 20px;
    line-height: 1.7;
  }

  .advanced code {
    padding: 1px 6px;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.25);
    user-select: all;
  }

  .client-id {
    display: flex;
    gap: 8px;
  }

  .client-id input {
    flex: 1;
    min-width: 0;
    padding: 7px 12px;
    border-radius: 10px;
    border: 1px solid var(--edge);
    background: var(--tint-1);
    outline: none;
    font-family: ui-monospace, "Cascadia Code", Consolas, monospace;
    font-size: 12.5px;
  }

  .client-id input:focus {
    border-color: var(--violet);
  }

  .hint {
    margin: 10px 0 0;
    color: var(--ink-2);
  }

  .version {
    margin: 28px 0 0;
    font-size: 12px;
    color: var(--ink-3);
  }
</style>

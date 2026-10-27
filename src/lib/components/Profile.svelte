<!-- Perfil: conta GlobalMusic e as três fontes de música (conectar/desconectar). Amigos têm aba própria. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import Copy from "@lucide/svelte/icons/copy";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import { api, errorMessage } from "$lib/api";
  import { app } from "$lib/app.svelte";
  import type { AccountStatus } from "$lib/types";
  import { SOURCE_LABEL } from "$lib/types";
  import SocialAccount from "./SocialAccount.svelte";
  import SourceIcon from "./SourceIcon.svelte";

  const REDIRECT_URI = "http://127.0.0.1:47823/login";
  const DASHBOARD_URL = "https://developer.spotify.com/dashboard/create";
  /** Client ID do Spotify: 32 caracteres hexadecimais. */
  const CLIENT_ID_FORMAT = /^[0-9a-f]{32}$/i;

  let copied = $state(false);

  async function copyRedirect() {
    await navigator.clipboard.writeText(REDIRECT_URI);
    copied = true;
    setTimeout(() => (copied = false), 2000);
  }

  // No navegador (demo) não existe o plugin do Tauri: abre numa aba.
  const openDashboard = () => openUrl(DASHBOARD_URL).catch(() => window.open(DASHBOARD_URL, "_blank"));

  // Client ID próprio do Spotify: cota de requisições só da pessoa (ver ADR 0003).
  let savedClientId = $state<string | null>(null);
  let clientIdInput = $state("");
  const clientIdValid = $derived(CLIENT_ID_FORMAT.test(clientIdInput.trim()));
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
    return `${who}, ${a.premium ? "Premium" : "Free"}`;
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
          O GlobalMusic usa um Client ID compartilhado com outros apps, e o Spotify divide o limite de requisições entre
          todos. Se aparecer "muitas requisições", crie um Client ID só seu (leva uns 2 minutos).
        </p>
        <p class="requirement">
          Precisa de <strong>Spotify Premium</strong>: desde fevereiro de 2026 o Spotify só deixa conta Premium criar
          Client ID. Quem é Free não precisa disto: continua usando o compartilhado, e as músicas tocam normalmente.
        </p>
        <ol class="steps">
          <li>
            <span>Abra o painel do Spotify (entre com a sua conta) e crie um app com qualquer nome.</span>
            <button class="btn ghost small" type="button" onclick={openDashboard}>
              <ExternalLink size={14} /> Abrir o painel do Spotify
            </button>
          </li>
          <li>
            <span>Em <em>Redirect URIs</em>, cole este endereço e clique em <em>Add</em>:</span>
            <span class="copy-row">
              <code>{REDIRECT_URI}</code>
              <button class="btn ghost small" type="button" onclick={copyRedirect}>
                <Copy size={14} /> {copied ? "Copiado" : "Copiar"}
              </button>
            </span>
          </li>
          <li><span>Em <em>Which API/SDKs</em>, marque <em>Web API</em> e <em>Web Playback SDK</em>, aceite os termos e salve.</span></li>
          <li><span>Abra o app criado, copie o <em>Client ID</em> e cole aqui:</span></li>
        </ol>
        <form
          class="client-id"
          onsubmit={(e) => {
            e.preventDefault();
            if (clientIdValid) void saveClientId(clientIdInput.trim());
          }}
        >
          <label class="sr-only" for="client-id">Client ID do Spotify</label>
          <input
            id="client-id"
            placeholder="32 letras e números, ex.: 1a2b3c…"
            bind:value={clientIdInput}
            spellcheck="false"
            aria-invalid={clientIdInput.trim() !== "" && !clientIdValid}
          />
          <button class="btn" type="submit" disabled={!clientIdValid}>Salvar</button>
          {#if savedClientId}
            <button class="btn ghost" type="button" onclick={() => saveClientId(null)}>Usar o compartilhado</button>
          {/if}
        </form>
        {#if clientIdInput.trim() !== "" && !clientIdValid}
          <p class="hint invalid" role="status">O Client ID tem 32 letras (a–f) e números. Confira se copiou inteiro.</p>
        {:else if clientIdMessage}
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

  .requirement {
    padding: 8px 12px;
    border-radius: 10px;
    background: var(--tint-1);
    color: var(--ink-2);
  }

  .steps {
    display: grid;
    gap: 10px;
    margin: 12px 0;
    padding-left: 20px;
  }

  .steps li span {
    display: block;
  }

  .steps li .btn {
    margin-top: 6px;
  }

  .copy-row {
    display: flex !important;
    align-items: center;
    gap: 8px;
    margin-top: 6px;
  }

  .copy-row .btn {
    margin-top: 0 !important;
  }

  .btn.small {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 10px;
    font-size: 12.5px;
  }

  .client-id input[aria-invalid="true"] {
    border-color: rgba(255, 90, 90, 0.6);
  }

  .hint.invalid {
    color: #ffb4b4;
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

<script lang="ts">
  import { onMount } from "svelte";
  import { isTauri } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { social } from "$lib/social.svelte";
  import { app } from "$lib/app.svelte";
  import Ambient from "$lib/components/Ambient.svelte";
  import Browser from "$lib/components/Browser.svelte";
  import NowPlaying from "$lib/components/NowPlaying.svelte";
  import Profile from "$lib/components/Profile.svelte";
  import TitleBar from "$lib/components/TitleBar.svelte";
  import UpdateCard from "$lib/components/UpdateCard.svelte";
  import QueuePanel from "$lib/components/QueuePanel.svelte";
  import FriendsPanel from "$lib/components/FriendsPanel.svelte";
  import ContextMenu from "$lib/components/ContextMenu.svelte";
  import VersionPicker from "$lib/components/VersionPicker.svelte";
  import NewPlaylist from "$lib/components/NewPlaylist.svelte";

  onMount(() => {
    void app.init();
    void social.init();
    void followWindowState();
  });

  /** Maximizada ou em tela cheia, a janela ocupa a tela toda: sem cantos arredondados (senão os
   *  cantos transparentes mostram o que está atrás da janela). */
  async function followWindowState() {
    if (!isTauri()) return;
    const win = getCurrentWindow();
    const update = async () => {
      const filled = (await win.isMaximized()) || (await win.isFullscreen());
      document.documentElement.dataset.filled = String(filled);
    };
    await update();
    await win.onResized(() => void update());
  }
</script>

<div class="window">
  <Ambient />
  <TitleBar />
  <main class:solo={!app.sidebarOpen}>
    {#if app.sidebarOpen}
      <Browser />
    {/if}
    <!-- A fila é um menu lateral por cima do player (que continua visível e se desloca para a esquerda). -->
    <div class="stage glass" class:queue-open={app.panel === "queue"}>
      {#if app.panel === "profile"}
        <Profile />
      {:else if app.panel === "friends"}
        <FriendsPanel />
      {:else}
        <NowPlaying />
      {/if}
      {#if app.panel === "queue"}
        <QueuePanel />
      {/if}
    </div>
  </main>
  <UpdateCard />
  <ContextMenu />
  <VersionPicker />
  {#if app.creating}
    <NewPlaylist initial={app.creating} />
  {/if}
  {#if app.toast}
    <div class="toast glass" role="status">{app.toast}</div>
  {/if}
</div>

<style>
  .toast {
    position: fixed;
    left: 50%;
    bottom: 24px;
    z-index: 20;
    transform: translateX(-50%);
    max-width: min(560px, 80vw);
    padding: 10px 16px;
    border-radius: 12px;
    font-size: 13px;
    background: linear-gradient(180deg, rgba(42, 39, 60, 0.97), rgba(30, 28, 42, 0.97));
  }

  .window {
    position: relative;
    height: 100%;
    display: flex;
    flex-direction: column;
    border-radius: var(--r-window);
    overflow: hidden;
  }

  main {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 360px 1fr;
    gap: 12px;
    padding: 0 12px 12px;
  }

  /* Painel da esquerda escondido: só o player. */
  main.solo {
    grid-template-columns: 1fr;
  }

  .stage {
    --drawer: min(380px, 46%);
    position: relative;
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }

  .stage > :global(*) {
    flex: 1;
  }

  /* Com a fila aberta, o player se centraliza no espaço que sobrou à esquerda. */
  .stage > :global(.now) {
    transition: padding-right 0.28s cubic-bezier(0.2, 0.8, 0.2, 1);
  }

  .stage.queue-open > :global(.now) {
    padding-right: calc(var(--drawer) + 24px);
  }

  @media (prefers-reduced-motion: reduce) {
    .stage > :global(.now) {
      transition: none;
    }
  }
</style>

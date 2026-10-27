<script lang="ts">
  import { onMount } from "svelte";
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

  onMount(() => {
    void app.init();
    void social.init();
  });
</script>

<div class="window">
  <Ambient />
  <TitleBar />
  <main>
    <Browser />
    <div class="stage glass">
      {#if app.panel === "profile"}
        <Profile />
      {:else if app.panel === "queue"}
        <QueuePanel />
      {:else if app.panel === "friends"}
        <FriendsPanel />
      {:else}
        <NowPlaying />
      {/if}
    </div>
  </main>
  <UpdateCard />
  <ContextMenu />
  <VersionPicker />
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

  .stage {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .stage > :global(*) {
    flex: 1;
  }
</style>

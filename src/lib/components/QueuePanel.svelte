<!-- Fila no modelo do Spotify: a sua fila (posta à mão) toca antes do resto da lista atual. -->
<script lang="ts">
  import { fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import X from "@lucide/svelte/icons/x";
  import { app } from "$lib/app.svelte";
  import type { QueueSlot, Track } from "$lib/types";
  import Bars from "./Bars.svelte";
  import TrackRow from "./TrackRow.svelte";

  const reduceMotion = typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
  const slide = { x: 48, duration: reduceMotion ? 0 : 260, easing: cubicOut, opacity: 0 };
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && (app.panel = "now")} />

{#snippet rows(items: { track: Track; slot: QueueSlot }[])}
  <ul>
    {#each items as { track, slot } (slot.list + slot.index + track.source + track.id)}
      <li>
        <TrackRow {track} onplay={() => app.jumpTo(slot)} />
        <button class="remove" aria-label="Tirar da fila" title="Tirar da fila" onclick={() => app.removeFromQueue(slot)}>
          <X size={15} />
        </button>
      </li>
    {/each}
  </ul>
{/snippet}

<aside class="queue" aria-label="Fila" transition:fly={slide}>
  <header>
    <h1>Fila</h1>
    <button class="close" aria-label="Fechar a fila" title="Fechar (Esc)" onclick={() => (app.panel = "now")}>
      <X size={17} />
    </button>
  </header>

  <div class="scroll">
    {#if app.player.current}
      <h2>Tocando agora</h2>
      <TrackRow track={app.player.current} current playing={app.player.playing} onplay={() => {}} />
    {/if}

    {#if app.upNext.length}
      <h2>Na sua fila</h2>
      {@render rows(app.upNext)}
    {/if}

    {#if app.contextRest.length}
      <h2>{app.player.contextName ? `A seguir de ${app.player.contextName}` : "A seguir"}</h2>
      {@render rows(app.contextRest)}
    {/if}

    {#if !app.upNext.length && !app.contextRest.length}
      <div class="empty"><Bars size={56} /><p>Nada depois desta música. Botão direito numa faixa: Adicionar à fila.</p></div>
    {/if}
  </div>
</aside>

<style>
  /* Menu lateral por cima do player, preso à direita do painel. */
  .queue {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 5;
    width: var(--drawer, 380px);
    display: flex;
    flex-direction: column;
    background: linear-gradient(180deg, rgba(36, 34, 52, 0.92), rgba(26, 25, 37, 0.94));
    border-left: 1px solid var(--edge);
    box-shadow: -24px 0 48px -24px rgba(0, 0, 0, 0.6);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 18px 16px 6px 20px;
  }

  h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 20px;
    font-weight: 650;
  }

  .close {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 10px;
    color: var(--ink-2);
  }

  .close:hover {
    background: var(--tint-2);
    color: var(--ink);
  }

  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0 10px 16px 12px;
  }

  h2 {
    margin: 16px 8px 6px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--ink-3);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  li {
    position: relative;
    display: flex;
    align-items: center;
  }

  li > :global(.row) {
    flex: 1;
    min-width: 0;
  }

  .remove {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    margin-left: 2px;
    border-radius: 8px;
    color: var(--ink-3);
    flex: none;
  }

  .remove:hover {
    color: var(--ink);
    background: var(--tint-2);
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 32px 16px;
    text-align: center;
    color: var(--ink-2);
  }

  .empty p {
    margin: 0;
    font-size: 13px;
  }
</style>

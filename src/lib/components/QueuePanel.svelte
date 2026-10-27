<!-- Fila no modelo do Spotify: a sua fila (posta à mão) toca antes do resto da lista atual. -->
<script lang="ts">
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import X from "@lucide/svelte/icons/x";
  import { app } from "$lib/app.svelte";
  import type { QueueSlot, Track } from "$lib/types";
  import Bars from "./Bars.svelte";
  import TrackRow from "./TrackRow.svelte";
</script>

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

<section class="queue" aria-label="Fila">
  <button class="back" onclick={() => (app.panel = "now")}><ChevronLeft size={16} /> Voltar ao player</button>
  <h1>Fila</h1>
  <p class="intro">
    Botão direito numa música: "Tocar a seguir" ou "Adicionar à fila". O que você adiciona toca antes do resto da
    lista, de qualquer fonte, e a lista continua de onde parou.
  </p>

  {#if app.player.current}
    <h2>Tocando agora</h2>
    <TrackRow track={app.player.current} current playing={app.player.playing} onplay={() => (app.panel = "now")} />
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
    <div class="empty"><Bars size={64} /><p>Nada depois desta música. Adicione faixas pelo botão direito.</p></div>
  {/if}
</section>

<style>
  .queue {
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
    margin: 24px 0 4px;
    font-family: var(--font-display);
    font-size: 28px;
    font-weight: 650;
  }

  .intro {
    margin: 0 0 8px;
    max-width: 60ch;
    color: var(--ink-2);
  }

  h2 {
    margin: 22px 0 6px;
    font-size: 13px;
    font-weight: 600;
    color: var(--ink-3);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    max-width: 720px;
  }

  li {
    position: relative;
    display: flex;
    align-items: center;
  }

  .remove {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    margin-left: 4px;
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
    align-items: flex-start;
    gap: 12px;
    padding: 12px 0;
    color: var(--ink-2);
  }

  .empty p {
    margin: 0;
  }
</style>

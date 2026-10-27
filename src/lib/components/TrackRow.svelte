<script lang="ts">
  import Heart from "@lucide/svelte/icons/heart";
  import { app } from "$lib/app.svelte";
  import type { Track } from "$lib/types";
  import Artwork from "./Artwork.svelte";
  import Bars from "./Bars.svelte";
  import SourceIcon from "./SourceIcon.svelte";

  let { track, current = false, playing = false, onplay }: {
    track: Track;
    current?: boolean;
    playing?: boolean;
    onplay: () => void;
  } = $props();

  // Mouse parado em cima por um instante = provável clique: prepara o áudio já.
  // (Só passar o mouse pela lista não dispara nada.)
  const HOVER_MS = 350;
  let hoverTimer: ReturnType<typeof setTimeout> | undefined;
  function hoverStart() {
    clearTimeout(hoverTimer);
    hoverTimer = setTimeout(() => app.prepareHover(track), HOVER_MS);
  }
  function hoverEnd() {
    clearTimeout(hoverTimer);
  }

  const NOW_ROWS: [number, number, number][] = [
    [0, 70, 100],
    [20, 80, 100],
    [0, 50, 76],
  ];
</script>

<!-- Botão direito: menu da faixa (curtir, fila, abrir no serviço). -->
<button
  class="row"
  class:current
  onclick={onplay}
  onpointerenter={hoverStart}
  onpointerleave={hoverEnd}
  onfocus={hoverStart}
  onblur={hoverEnd}
  oncontextmenu={(e) => app.openMenu(track, e)}
  aria-current={current ? "true" : undefined}
>
  <Artwork url={track.artwork?.thumb ?? null} seed={track.id} size="44px" radius="10px" />
  <span class="text">
    <span class="title">{track.title}</span>
    <span class="artist">{track.artists.join(", ")}</span>
  </span>
  {#if current}
    <Bars rows={NOW_ROWS} size={22} active={playing} />
  {/if}
  {#if app.isFavorite(track)}
    <span class="liked" title="Curtida no GlobalMusic"><Heart size={13} fill="currentColor" strokeWidth={0} /></span>
  {/if}
  <SourceIcon source={track.source} size={14} />
</button>

<style>
  .liked {
    display: grid;
    color: var(--violet-bright);
  }

  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 7px 10px 7px 7px;
    border-radius: var(--r-row);
    text-align: left;
  }

  .row:hover {
    background: var(--tint-1);
  }

  .row.current {
    background: var(--tint-3);
  }

  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .title,
  .artist {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .title {
    font-weight: 600;
  }

  .artist {
    font-size: 12.5px;
    color: var(--ink-2);
  }
</style>

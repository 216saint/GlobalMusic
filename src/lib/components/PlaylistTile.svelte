<script lang="ts">
  import type { Playlist } from "$lib/types";
  import { SOURCE_LABEL, isLikedCollection } from "$lib/types";
  import Artwork from "./Artwork.svelte";
  import LikedCover from "./LikedCover.svelte";
  import SourceIcon from "./SourceIcon.svelte";

  import X from "@lucide/svelte/icons/x";

  let { playlist, onopen, onremove }: { playlist: Playlist; onopen: () => void; onremove?: () => void } = $props();
</script>

<div class="wrap">
<button class="tile" onclick={onopen} title="{playlist.name} — {SOURCE_LABEL[playlist.source]}">
  <span class="cover">
    {#if isLikedCollection(playlist)}
      <LikedCover source={playlist.source} />
    {:else}
      <Artwork url={playlist.artwork?.thumb ?? null} seed={playlist.id} size="100%" radius="14px" />
    {/if}
    <!-- Selo de origem: canto inferior direito, como pedido. -->
    <span class="badge"><SourceIcon source={playlist.source} size={16} /></span>
  </span>
  <span class="name">{playlist.name}</span>
  {#if playlist.trackCount !== null}
    <span class="meta tabular">{playlist.trackCount} músicas</span>
  {:else if onremove}
    <span class="meta">Importada</span>
  {/if}
</button>
{#if onremove}
  <!-- Playlist importada por link: dá para tirar da biblioteca. -->
  <button class="remove" aria-label="Tirar {playlist.name} da biblioteca" title="Tirar da biblioteca" onclick={onremove}>
    <X size={14} />
  </button>
{/if}
</div>

<style>
  .wrap {
    position: relative;
    min-width: 0;
  }

  .remove {
    position: absolute;
    top: 14px;
    right: 14px;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    background: rgba(14, 13, 20, 0.78);
    color: var(--ink);
    opacity: 0;
    transition: opacity 0.15s;
  }

  .wrap:hover .remove,
  .remove:focus-visible {
    opacity: 1;
  }

  .remove:hover {
    background: #c42b1c;
  }

  .tile {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px;
    border-radius: 16px;
    text-align: left;
    min-width: 0;
  }

  .tile:hover {
    background: var(--tint-1);
  }

  /* Quadrado fixo: uma foto alta não pode esticar o card (o mínimo automático de um item
     flex deixaria a imagem "empurrar" a altura). A imagem é recortada dentro dele. */
  .cover {
    position: relative;
    width: 100%;
    aspect-ratio: 1;
    min-height: 0;
    overflow: hidden;
    border-radius: 14px;
    margin-bottom: 8px;
  }

  .cover > :global(:first-child) {
    position: absolute;
    inset: 0;
  }

  .badge {
    position: absolute;
    right: 7px;
    bottom: 7px;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: rgba(14, 13, 20, 0.72);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.35);
  }

  .name {
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .meta {
    font-size: 12px;
    color: var(--ink-3);
  }
</style>

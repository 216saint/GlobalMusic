<script lang="ts">
  import type { Playlist } from "$lib/types";
  import { SOURCE_LABEL } from "$lib/types";
  import Artwork from "./Artwork.svelte";
  import SourceIcon from "./SourceIcon.svelte";

  let { playlist, onopen }: { playlist: Playlist; onopen: () => void } = $props();
</script>

<button class="tile" onclick={onopen} title="{playlist.name} — {SOURCE_LABEL[playlist.source]}">
  <span class="cover">
    <Artwork url={playlist.artwork?.thumb ?? null} seed={playlist.id} size="100%" radius="14px" />
    <!-- Selo de origem: canto inferior direito, como pedido. -->
    <span class="badge"><SourceIcon source={playlist.source} size={16} /></span>
  </span>
  <span class="name">{playlist.name}</span>
  {#if playlist.trackCount !== null}
    <span class="meta tabular">{playlist.trackCount} músicas</span>
  {/if}
</button>

<style>
  .tile {
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

  .cover {
    position: relative;
    aspect-ratio: 1;
    margin-bottom: 8px;
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

<!-- Card de uma playlist do GlobalMusic na biblioteca. Apagar pede confirmação no próprio card. -->
<script lang="ts">
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import { app } from "$lib/app.svelte";
  import type { LocalPlaylist } from "$lib/types";
  import Bars from "./Bars.svelte";
  import LocalCover from "./LocalCover.svelte";

  let { playlist }: { playlist: LocalPlaylist } = $props();

  let confirming = $state(false);
</script>

<div class="wrap" role="presentation" onpointerleave={() => (confirming = false)}>
  <button
    class="tile"
    onclick={() => app.openLocal(playlist)}
    oncontextmenu={(e) => app.openPlaylistMenu({ kind: "local", playlist }, e)}
    title={playlist.name}
  >
    <span class="cover">
      <LocalCover tracks={playlist.tracks} cover={playlist.cover} />
      <span class="badge" title="Playlist do GlobalMusic"><Bars size={14} /></span>
    </span>
    <span class="name">{playlist.name}</span>
    <span class="meta tabular">{playlist.tracks.length} {playlist.tracks.length === 1 ? "música" : "músicas"}</span>
  </button>
  {#if confirming}
    <button class="delete confirm" onclick={() => app.deletePlaylist(playlist)}>Apagar?</button>
  {:else}
    <button class="delete" aria-label="Apagar {playlist.name}" title="Apagar playlist" onclick={() => (confirming = true)}>
      <Trash2 size={13} />
    </button>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
    min-width: 0;
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

  .delete {
    position: absolute;
    top: 14px;
    right: 14px;
    display: grid;
    place-items: center;
    min-width: 26px;
    height: 26px;
    padding: 0 8px;
    border-radius: 999px;
    background: rgba(14, 13, 20, 0.78);
    color: var(--ink);
    font-size: 12px;
    font-weight: 600;
    opacity: 0;
    transition: opacity 0.15s;
  }

  .wrap:hover .delete,
  .delete:focus-visible,
  .delete.confirm {
    opacity: 1;
  }

  .delete:hover,
  .delete.confirm {
    background: #c42b1c;
  }
</style>

<!-- Capa de uma playlist do GlobalMusic: mosaico com as capas das 4 primeiras músicas (como o
     Spotify faz). Com menos de 4 capas, a primeira ocupa tudo; sem nenhuma, as barras do logo. -->
<script lang="ts">
  import type { Track } from "$lib/types";
  import Bars from "./Bars.svelte";

  let { tracks, size = "100%", radius = "14px" }: { tracks: Track[]; size?: string; radius?: string } = $props();

  const covers = $derived(
    [...new Set(tracks.map((t) => t.artwork?.thumb).filter((url): url is string => !!url))].slice(0, 4),
  );
</script>

<span class="cover" style:width={size} style:height={size} style:border-radius={radius}>
  {#if covers.length >= 4}
    {#each covers as url (url)}
      <img src={url} alt="" loading="lazy" decoding="async" />
    {/each}
  {:else if covers.length}
    <img class="single" src={covers[0]} alt="" loading="lazy" decoding="async" />
  {:else}
    <span class="empty"><Bars size={44} /></span>
  {/if}
</span>

<style>
  .cover {
    display: grid;
    grid-template-columns: 1fr 1fr;
    grid-template-rows: 1fr 1fr;
    flex: none;
    overflow: hidden;
    background: linear-gradient(145deg, #2b2547, var(--graphite));
  }

  img {
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    object-fit: cover;
    display: block;
  }

  .single {
    grid-column: 1 / -1;
    grid-row: 1 / -1;
  }

  .empty {
    grid-column: 1 / -1;
    grid-row: 1 / -1;
    display: grid;
    place-items: center;
  }
</style>

<!-- Detalhe de uma lista da biblioteca (playlist de um serviço ou curtidas do GlobalMusic).
     Clicar numa faixa toca a lista a partir dela; botão direito abre o menu da faixa. -->
<script lang="ts">
  import Heart from "@lucide/svelte/icons/heart";
  import Play from "@lucide/svelte/icons/play";
  import { app, type OpenedPlaylist } from "$lib/app.svelte";
  import { SOURCE_LABEL } from "$lib/types";
  import Artwork from "./Artwork.svelte";
  import Bars from "./Bars.svelte";
  import SourceIcon from "./SourceIcon.svelte";
  import TrackRow from "./TrackRow.svelte";

  let { opened }: { opened: OpenedPlaylist } = $props();

  const isCurrent = (id: string, source: string) => app.player.current?.id === id && app.player.current?.source === source;
</script>

<div class="detail">
  <header>
    {#if opened.kind === "favorites"}
      <span class="fav-art"><Heart size={30} fill="currentColor" strokeWidth={0} /></span>
    {:else}
      <Artwork url={opened.artworkUrl} seed={opened.id} size="72px" radius="14px" />
    {/if}
    <div class="meta">
      <h2>{opened.title}</h2>
      <p>
        {#if opened.source}
          <SourceIcon source={opened.source} size={12} />
          {SOURCE_LABEL[opened.source]}
        {:else}
          Todas as fontes
        {/if}
        {#if !opened.loading && !opened.error} · {opened.tracks.length} músicas{/if}
      </p>
    </div>
    {#if opened.tracks.length}
      <button class="play" aria-label="Tocar a lista" onclick={() => app.play(opened.tracks, 0, opened.title)}>
        <Play size={18} strokeWidth={2} />
      </button>
    {/if}
  </header>

  {#if opened.loading}
    <div class="state"><Bars size={64} active /><p>Carregando as faixas…</p></div>
  {:else if opened.error}
    <p class="error" role="alert">{opened.error}</p>
  {:else if !opened.tracks.length}
    <div class="state">
      <p>{opened.kind === "favorites" ? "Clique com o botão direito numa música e escolha Curtir." : "Esta playlist está vazia."}</p>
    </div>
  {:else}
    <ul class="tracks">
      {#each opened.tracks as track, i (track.source + track.id + i)}
        {@const current = isCurrent(track.id, track.source)}
        <li>
          <TrackRow {track} {current} playing={current && app.player.playing} onplay={() => app.play(opened.tracks, i, opened.title)} />
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 6px 12px;
  }

  .fav-art {
    display: grid;
    place-items: center;
    width: 72px;
    height: 72px;
    flex: none;
    border-radius: 14px;
    color: #fff;
    background: linear-gradient(135deg, var(--violet-deep), var(--violet-bright));
  }

  .meta {
    flex: 1;
    min-width: 0;
  }

  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 17px;
    font-weight: 650;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .meta p {
    display: flex;
    align-items: center;
    gap: 5px;
    margin: 3px 0 0;
    font-size: 12px;
    color: var(--ink-3);
  }

  .play {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    background: var(--violet);
    color: #fff;
    flex: none;
  }

  .play:hover {
    background: var(--violet-bright);
  }

  .tracks {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    padding: 36px 16px;
    color: var(--ink-2);
    text-align: center;
  }

  .state p {
    margin: 0;
  }

  .error {
    margin: 4px 6px;
    padding: 10px 12px;
    border-radius: 10px;
    background: rgba(255, 90, 90, 0.12);
    border: 1px solid rgba(255, 90, 90, 0.3);
    color: #ffd5d5;
    font-size: 12.5px;
  }
</style>

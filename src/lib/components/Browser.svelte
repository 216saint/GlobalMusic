<!-- Painel esquerdo: busca federada e biblioteca unificada. -->
<script lang="ts">
  import Search from "@lucide/svelte/icons/search";
  import Heart from "@lucide/svelte/icons/heart";
  import ArrowLeft from "@lucide/svelte/icons/arrow-left";
  import { app } from "$lib/app.svelte";
  import type { SourceResults } from "$lib/types";
  import { SOURCE_LABEL } from "$lib/types";
  import Bars from "./Bars.svelte";
  import PlaylistTile from "./PlaylistTile.svelte";
  import PlaylistView from "./PlaylistView.svelte";
  import SourceIcon from "./SourceIcon.svelte";
  import TrackRow from "./TrackRow.svelte";

  const SEARCH_DEBOUNCE_MS = 280;

  let timer: ReturnType<typeof setTimeout> | undefined;
  function onInput(value: string) {
    app.query = value;
    clearTimeout(timer);
    timer = setTimeout(() => app.search(value), SEARCH_DEBOUNCE_MS);
  }

  const failures = $derived(app.failures((app.tab === "search" ? app.results : app.playlists) as SourceResults<unknown>[]));

  const capitalize = (text: string) => text.charAt(0).toUpperCase() + text.slice(1);

  const routeLabel = $derived.by(() => {
    const route = app.player.route?.kind;
    if (!route) return "Nada tocando";
    if (app.player.loading) return "Carregando o áudio…";
    if (route === "spotify_native") return "Tocando pelo Spotify Premium";
    if (route === "youtube_fallback") return "Spotify Free: áudio vindo do YouTube";
    return `Tocando pelo ${SOURCE_LABEL[app.player.current!.source]}`;
  });
</script>

<!-- Abas (Buscar/Biblioteca) e filtros de fonte ficam na barra de título (TitleBar.svelte). -->
<section class="browser glass" aria-label={app.tab === "search" ? "Buscar" : "Biblioteca"}>
  <h2 class="section">
    <!-- Dentro de uma playlist, o título vira o caminho de volta (como no Spotify). -->
    {#if app.tab === "library" && app.opened}
      <button class="back" aria-label="Voltar para a Biblioteca" title="Voltar para a Biblioteca" onclick={() => app.closePlaylist()}>
        <ArrowLeft size={17} strokeWidth={2} />
      </button>
    {/if}
    {app.tab === "search" ? "Buscar" : "Biblioteca"}
    {#if app.filter !== "all"}<span class="scope"><SourceIcon source={app.filter} size={12} /> {SOURCE_LABEL[app.filter]}</span>{/if}
  </h2>

  {#if app.tab === "search"}
    <label class="search">
      <Search size={16} strokeWidth={1.8} />
      <span class="sr-only">Buscar música</span>
      <input
        type="search"
        placeholder="Buscar em todas as fontes"
        value={app.query}
        oninput={(e) => onInput(e.currentTarget.value)}
        spellcheck="false"
      />
    </label>
  {/if}

  <div class="scroll">
    {#each app.tab === "library" && app.opened ? [] : failures as f (f.source)}
      <p class="failure">
        <SourceIcon source={f.source} size={14} />
        <span>{SOURCE_LABEL[f.source]}: {f.error}</span>
      </p>
    {/each}

    {#if app.tab === "search"}
      {#if app.tracks.length}
        <ul class="tracks">
          {#each app.tracks as track, i (track.source + track.id)}
            {@const current = app.player.current?.id === track.id && app.player.current?.source === track.source}
            <li>
              <TrackRow {track} {current} playing={current && app.player.playing} onplay={() => app.play(app.tracks, i, `Busca: ${app.query}`)} />
            </li>
          {/each}
        </ul>
      {:else if !app.query.trim()}
        <div class="empty">
          <Bars size={96} />
          <p class="lead">Busque uma vez, ouça de qualquer lugar.</p>
          <p>Cada busca procura no Spotify, no YouTube Music e no SoundCloud ao mesmo tempo.</p>
        </div>
      {:else if !app.searching}
        <div class="empty"><p>Nada encontrado para “{app.query}”.</p></div>
      {/if}
    {:else if app.opened}
      <PlaylistView opened={app.opened} />
    {:else if app.playlistItems.length || app.favorites.length}
      {#if app.filter === "all"}
        <button class="favorites" onclick={() => app.openFavorites()}>
          <span class="fav-icon"><Heart size={18} fill="currentColor" strokeWidth={0} /></span>
          <span class="fav-text">
            <strong>Curtidas no GlobalMusic</strong>
            <span>{app.favorites.length} músicas de todas as fontes</span>
          </span>
        </button>
      {/if}
      <div class="grid">
        {#each app.playlistItems as playlist (playlist.source + playlist.id)}
          <PlaylistTile {playlist} onopen={() => app.openPlaylist(playlist)} />
        {/each}
      </div>
    {:else}
      <div class="empty">
        <Bars size={96} />
        <p class="lead">Sua biblioteca aparece aqui.</p>
        <p>Conecte suas contas para juntar as playlists de todas as fontes.</p>
      </div>
    {/if}
  </div>

  <footer class="status">
    {#if app.playbackError}
      <!-- A mensagem inteira fica no title: passe o mouse para ler o que o rodapé cortou. -->
      <span class="error" role="status" title={app.playbackError}>{capitalize(app.playbackError)}</span>
    {:else}
      <span>{routeLabel}</span>
    {/if}
    {#if app.searching}<span class="searching">Buscando…</span>{/if}
  </footer>
</section>

<style>
  .browser {
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 14px 10px 0;
  }

  .section {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 2px 6px 0;
    font-family: var(--font-display);
    font-size: 17px;
    font-weight: 650;
  }

  .back {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    margin-left: -4px;
    border-radius: 50%;
    background: var(--tint-1);
    border: 1px solid var(--edge);
    color: var(--ink-2);
  }

  .back:hover {
    background: var(--tint-3);
    color: var(--ink);
  }

  .scope {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-family: var(--font);
    font-size: 12px;
    font-weight: 500;
    color: var(--ink-3);
  }

  .favorites {
    display: flex;
    align-items: center;
    gap: 12px;
    width: calc(100% - 16px);
    margin: 0 8px 6px;
    padding: 10px;
    border-radius: 14px;
    text-align: left;
    background: var(--tint-1);
    border: 1px solid var(--edge);
  }

  .favorites:hover {
    background: var(--tint-2);
  }

  .fav-icon {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    flex: none;
    border-radius: 10px;
    color: #fff;
    background: linear-gradient(135deg, var(--violet-deep), var(--violet-bright));
  }

  .fav-text {
    display: flex;
    flex-direction: column;
  }

  .fav-text span {
    font-size: 12px;
    color: var(--ink-3);
  }

  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 12px 4px 4px;
    padding: 0 12px;
    height: 38px;
    border-radius: 12px;
    background: var(--tint-1);
    border: 1px solid var(--edge);
    color: var(--ink-3);
  }

  .search:focus-within {
    border-color: var(--violet);
    color: var(--ink-2);
  }

  .search input {
    flex: 1;
    min-width: 0;
    background: none;
    border: 0;
    outline: none;
  }

  .search input::placeholder {
    color: var(--ink-3);
  }

  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    margin-top: 8px;
    padding-bottom: 8px;
    scrollbar-width: thin;
    scrollbar-color: var(--tint-3) transparent;
  }

  .tracks {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(128px, 1fr));
  }

  .failure {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 4px 6px 8px;
    padding: 8px 10px;
    border-radius: 10px;
    background: rgba(0, 0, 0, 0.16);
    font-size: 12.5px;
    color: var(--ink-2);
  }

  .failure span {
    flex: 1;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    padding: 48px 28px;
    color: var(--ink-2);
  }

  .empty p {
    margin: 0;
    max-width: 32ch;
  }

  .empty .lead {
    margin: 22px 0 6px;
    color: var(--ink);
    font-family: var(--font-display);
    font-size: 16px;
    font-weight: 600;
  }

  .status {
    display: flex;
    justify-content: space-between;
    padding: 10px 8px 12px;
    font-size: 12px;
    color: var(--ink-3);
    border-top: 1px solid var(--tint-1);
  }

  .searching {
    color: var(--violet-bright);
  }

  .error {
    color: #ffb4b4;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>

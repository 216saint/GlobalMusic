<!-- Janela "Nova playlist" (aberta pelo card "+" da biblioteca ou pelo menu de uma faixa):
     - Criar: uma playlist do GlobalMusic, com sugestões do que tocou por último e da última busca;
     - Importar: uma playlist do Spotify, do YouTube Music ou do SoundCloud pelo link. -->
<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import X from "@lucide/svelte/icons/x";
  import Plus from "@lucide/svelte/icons/plus";
  import Check from "@lucide/svelte/icons/check";
  import { app } from "$lib/app.svelte";
  import { SOURCES, SOURCE_LABEL, type Track } from "$lib/types";
  import Artwork from "./Artwork.svelte";
  import SourceIcon from "./SourceIcon.svelte";

  let { initial }: { initial: Track[] } = $props();

  type Mode = "create" | "import";
  let mode = $state<Mode>("create");
  let name = $state("");
  let link = $state("");
  let busy = $state(false);
  // svelte-ignore state_referenced_locally
  let picked = $state<Track[]>([...initial]);

  const key = (t: Track) => `${t.source}:${t.id}`;
  const isPicked = (t: Track) => picked.some((p) => key(p) === key(t));
  const toggle = (t: Track) => (picked = isPicked(t) ? picked.filter((p) => key(p) !== key(t)) : [...picked, t]);

  // As duas listas de sugestão, sem repetir entre si (o que já está nas recentes some da busca).
  const recent = $derived(app.recent.slice(0, 12));
  const fromSearch = $derived(
    app.lastSearch.tracks.filter((t) => !recent.some((r) => key(r) === key(t))).slice(0, 12),
  );

  const close = () => (app.creating = null);
  /** Foco no campo ao abrir (o `autofocus` do HTML só vale no carregamento da página). */
  const focus = (input: HTMLInputElement) => input.focus();
  // Músicas que já vieram marcadas (ex.: do menu da faixa) e não estão nas sugestões: ficam numa
  // lista própria, para continuar dando para desmarcar e marcar de novo.
  const extra = $derived(initial.filter((t) => !recent.some((r) => key(r) === key(t)) && !fromSearch.some((r) => key(r) === key(t))));

  async function create(event: SubmitEvent) {
    event.preventDefault();
    if (!name.trim() || busy) return;
    busy = true;
    await app.createPlaylist(name, picked);
    busy = false;
  }

  async function importLink(event: SubmitEvent) {
    event.preventDefault();
    if (await app.importPlaylist(link)) close();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<div class="backdrop" role="presentation" onclick={close} transition:fade={{ duration: 140 }}></div>
<div class="dialog glass" role="dialog" aria-modal="true" aria-labelledby="new-playlist-title" transition:scale={{ start: 0.96, duration: 160 }}>
  <header>
    <h2 id="new-playlist-title">Nova playlist</h2>
    <button class="close" aria-label="Fechar" onclick={close}><X size={17} /></button>
  </header>

  <div class="modes" role="tablist">
    <button role="tab" aria-selected={mode === "create"} class:on={mode === "create"} onclick={() => (mode = "create")}>
      Criar no GlobalMusic
    </button>
    <button role="tab" aria-selected={mode === "import"} class:on={mode === "import"} onclick={() => (mode = "import")}>
      Importar por link
    </button>
  </div>

  {#if mode === "create"}
    <form class="body" onsubmit={create}>
      <label class="field">
        <span>Nome</span>
        <input bind:value={name} placeholder="Minha playlist" maxlength="100" spellcheck="false" use:focus />
      </label>

      <div class="suggestions">
        {#if extra.length}
          <h3>Selecionadas</h3>
          {@render rows(extra)}
        {/if}
        {#if recent.length}
          <h3>Tocadas recentemente</h3>
          {@render rows(recent)}
        {/if}
        {#if fromSearch.length}
          <h3>Da sua última busca: “{app.lastSearch.query}”</h3>
          {@render rows(fromSearch)}
        {/if}
        {#if !recent.length && !fromSearch.length && !extra.length}
          <p class="hint">
            Crie a playlist vazia e adicione músicas depois: clique com o botão direito numa música e escolha
            "Adicionar à playlist".
          </p>
        {/if}
      </div>

      <footer>
        <button class="primary" type="submit" disabled={!name.trim() || busy}>
          Criar playlist{picked.length ? ` com ${picked.length} ${picked.length === 1 ? "música" : "músicas"}` : ""}
        </button>
      </footer>
    </form>
  {:else}
    <form class="body" onsubmit={importLink}>
      <p class="lead">Cole o link de uma playlist pública. Ela entra na sua biblioteca, mesmo sem conta conectada.</p>
      <div class="sources">
        {#each SOURCES as source}
          <span class="source"><SourceIcon {source} size={15} /> {SOURCE_LABEL[source]}</span>
        {/each}
      </div>
      <label class="field">
        <span>Link da playlist</span>
        <input bind:value={link} placeholder="https://open.spotify.com/playlist/…" spellcheck="false" use:focus />
      </label>
      <footer>
        <button class="primary" type="submit" disabled={!link.trim() || app.importing}>
          {app.importing ? "Importando…" : "Importar playlist"}
        </button>
      </footer>
    </form>
  {/if}
</div>

{#snippet rows(tracks: Track[])}
  <ul>
    {#each tracks as track (key(track))}
      {@const on = isPicked(track)}
      <li>
        <button type="button" class="pick" class:on onclick={() => toggle(track)} aria-pressed={on}>
          <Artwork url={track.artwork?.thumb ?? null} seed={track.id} size="36px" radius="8px" />
          <span class="text">
            <span class="title">{track.title}</span>
            <span class="artist">{track.artists.join(", ")}</span>
          </span>
          <SourceIcon source={track.source} size={13} />
          <span class="mark">
            {#if on}<Check size={14} strokeWidth={2.4} />{:else}<Plus size={14} strokeWidth={2.2} />{/if}
          </span>
        </button>
      </li>
    {/each}
  </ul>
{/snippet}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: rgba(8, 7, 14, 0.55);
  }

  .dialog {
    position: fixed;
    z-index: 41;
    left: 50%;
    top: 50%;
    translate: -50% -50%;
    width: min(520px, calc(100vw - 48px));
    max-height: min(640px, calc(100vh - 96px));
    display: flex;
    flex-direction: column;
    padding: 18px 18px 16px;
    background: linear-gradient(180deg, rgba(42, 39, 60, 0.98), rgba(26, 24, 37, 0.98));
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 18px;
    font-weight: 650;
  }

  .close {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    color: var(--ink-2);
  }

  .close:hover {
    background: var(--tint-2);
    color: var(--ink);
  }

  .modes {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px;
    margin: 14px 0 4px;
    padding: 4px;
    border-radius: 12px;
    background: rgba(0, 0, 0, 0.22);
  }

  .modes button {
    padding: 7px 10px;
    border-radius: 9px;
    font-size: 13px;
    font-weight: 600;
    color: var(--ink-2);
  }

  .modes button.on {
    background: var(--violet);
    color: #fff;
  }

  .body {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 12px;
    font-size: 12px;
    color: var(--ink-3);
  }

  .field input {
    padding: 10px 12px;
    border-radius: 11px;
    border: 1px solid var(--edge);
    background: var(--tint-1);
    outline: none;
    font-size: 14px;
  }

  .field input:focus {
    border-color: var(--violet);
  }

  .suggestions {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    margin: 12px -6px 0;
    padding: 0 6px;
  }

  h3 {
    margin: 10px 4px 4px;
    font-size: 12px;
    font-weight: 600;
    color: var(--ink-3);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .pick {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 6px;
    border-radius: 10px;
    text-align: left;
  }

  .pick:hover {
    background: var(--tint-1);
  }

  .text {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }

  .title,
  .artist {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .title {
    font-size: 13px;
    font-weight: 600;
  }

  .artist {
    font-size: 12px;
    color: var(--ink-3);
  }

  .mark {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    flex: none;
    border-radius: 50%;
    border: 1px solid var(--edge);
    color: var(--ink-2);
  }

  .pick.on .mark {
    border-color: transparent;
    background: var(--violet);
    color: #fff;
  }

  .hint,
  .lead {
    margin: 12px 4px 0;
    font-size: 13px;
    color: var(--ink-2);
  }

  .sources {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 12px 0 0;
  }

  .source {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 10px;
    border-radius: 999px;
    background: var(--tint-1);
    border: 1px solid var(--edge);
    font-size: 12px;
    color: var(--ink-2);
  }

  footer {
    display: flex;
    justify-content: flex-end;
    padding-top: 14px;
  }

  .primary {
    padding: 9px 16px;
    border-radius: 11px;
    font-size: 13px;
    font-weight: 650;
    background: var(--violet);
    color: #fff;
  }

  .primary:hover:not(:disabled) {
    background: var(--violet-bright);
  }

  .primary:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>

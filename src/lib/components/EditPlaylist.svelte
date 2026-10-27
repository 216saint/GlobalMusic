<!-- Janela "Editar playlist": nome e foto de uma playlist do GlobalMusic. A foto é recortada no
     centro e reduzida para 512 px no próprio app (ver `image.ts`). -->
<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import X from "@lucide/svelte/icons/x";
  import ImagePlus from "@lucide/svelte/icons/image-plus";
  import { app } from "$lib/app.svelte";
  import { squareJpegDataUrl } from "$lib/image";
  import type { LocalPlaylist } from "$lib/types";
  import LocalCover from "./LocalCover.svelte";

  const COVER_PX = 512;

  let { playlist }: { playlist: LocalPlaylist } = $props();

  // svelte-ignore state_referenced_locally
  let name = $state(playlist.name);
  // svelte-ignore state_referenced_locally
  let cover = $state<string | null>(playlist.cover ?? null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let fileInput = $state<HTMLInputElement>();

  const close = () => (app.editing = null);
  const focus = (input: HTMLInputElement) => input.select();

  async function pick(event: Event) {
    const file = (event.currentTarget as HTMLInputElement).files?.[0];
    if (!file) return;
    error = null;
    try {
      cover = await squareJpegDataUrl(file, COVER_PX);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (!name.trim() || busy) return;
    busy = true;
    const changes: { name?: string; cover?: string; removeCover?: boolean } = {};
    if (name.trim() !== playlist.name) changes.name = name;
    if (cover !== (playlist.cover ?? null)) {
      if (cover) changes.cover = cover;
      else changes.removeCover = true;
    }
    if (!Object.keys(changes).length || (await app.editPlaylist(playlist, changes))) close();
    busy = false;
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<div class="backdrop" role="presentation" onclick={close} transition:fade={{ duration: 140 }}></div>
<div
  class="dialog glass"
  role="dialog"
  aria-modal="true"
  aria-labelledby="edit-playlist-title"
  transition:scale={{ start: 0.96, duration: 160 }}
>
<form onsubmit={save}>
  <header>
    <h2 id="edit-playlist-title">Editar playlist</h2>
    <button type="button" class="close" aria-label="Fechar" onclick={close}><X size={17} /></button>
  </header>

  <div class="body">
    <!-- Clicar na capa escolhe a foto. -->
    <button type="button" class="cover" onclick={() => fileInput?.click()} title="Escolher foto">
      <LocalCover tracks={playlist.tracks} {cover} size="148px" radius="16px" />
      <span class="overlay"><ImagePlus size={22} /> Escolher foto</span>
    </button>
    <input bind:this={fileInput} type="file" accept="image/*" hidden onchange={pick} />

    <div class="side">
      <label class="field">
        <span>Nome</span>
        <input bind:value={name} maxlength="100" spellcheck="false" use:focus />
      </label>
      {#if cover}
        <button type="button" class="link" onclick={() => (cover = null)}>Tirar a foto (volta o mosaico das músicas)</button>
      {/if}
      {#if error}<p class="error" role="alert">{error}</p>{/if}
    </div>
  </div>

  <footer>
    <button type="button" class="ghost" onclick={close}>Cancelar</button>
    <button class="primary" type="submit" disabled={!name.trim() || busy}>Salvar</button>
  </footer>
</form>
</div>

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
    width: min(500px, calc(100vw - 48px));
    padding: 18px;
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

  .body {
    display: flex;
    gap: 18px;
    margin-top: 16px;
  }

  .cover {
    position: relative;
    flex: none;
    border-radius: 16px;
    overflow: hidden;
  }

  .overlay {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    font-size: 12.5px;
    font-weight: 600;
    color: #fff;
    background: rgba(14, 13, 20, 0.6);
    opacity: 0;
    transition: opacity 0.15s;
  }

  .cover:hover .overlay,
  .cover:focus-visible .overlay {
    opacity: 1;
  }

  .side {
    flex: 1;
    min-width: 0;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
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

  .link {
    margin-top: 12px;
    font-size: 12.5px;
    color: var(--violet-bright);
    text-align: left;
  }

  .link:hover {
    text-decoration: underline;
  }

  .error {
    margin: 10px 0 0;
    font-size: 12.5px;
    color: #ffb4b4;
  }

  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 18px;
  }

  .primary,
  .ghost {
    padding: 9px 16px;
    border-radius: 11px;
    font-size: 13px;
    font-weight: 650;
  }

  .primary {
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

  .ghost {
    border: 1px solid var(--edge);
    color: var(--ink-2);
  }

  .ghost:hover {
    color: var(--ink);
  }
</style>

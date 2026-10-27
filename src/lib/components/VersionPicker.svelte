<!-- "Escolher versão do YouTube": quando a música do Spotify toca pelo YouTube e a busca escolheu
     a versão errada (ao vivo, cover…), a pessoa escolhe a certa. A escolha vale para sempre. -->
<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import { app } from "$lib/app.svelte";
  import Artwork from "./Artwork.svelte";

  const picker = $derived(app.versions);

  function time(ms: number) {
    const s = Math.round(ms / 1000);
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && (app.versions = null)} />

{#if picker}
  <div class="backdrop" role="presentation" onclick={() => (app.versions = null)}></div>
  <div class="dialog glass" role="dialog" aria-modal="true" aria-labelledby="versions-title">
    <header>
      <div>
        <h2 id="versions-title">Escolher versão do YouTube</h2>
        <p>
          "{picker.track.title}" ({time(picker.track.durationMs)}) toca pelo YouTube. Se a versão estiver errada,
          escolha a certa.
        </p>
      </div>
      <button class="close" aria-label="Fechar" onclick={() => (app.versions = null)}><X size={16} /></button>
    </header>

    {#if picker.loading}
      <p class="status">Buscando versões…</p>
    {:else if picker.error}
      <p class="status">{picker.error}</p>
    {:else if !picker.options.length}
      <p class="status">Nenhuma versão encontrada no YouTube.</p>
    {:else}
      <ul>
        {#each picker.options as option (option.id)}
          {@const inUse = option.id === picker.current}
          <li>
            <button class="option" class:in-use={inUse} disabled={inUse} onclick={() => app.chooseVersion(option)}>
              <Artwork url={option.artwork?.thumb ?? null} seed={option.id} size="40px" radius="8px" />
              <span class="text">
                <strong>{option.title}</strong>
                <span>{option.artists.join(", ")} · {time(option.durationMs)}</span>
              </span>
              {#if inUse}<span class="badge">Em uso</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: rgba(0, 0, 0, 0.35);
  }

  .dialog {
    position: fixed;
    z-index: 41;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: min(520px, calc(100vw - 32px));
    max-height: min(560px, calc(100vh - 80px));
    display: flex;
    flex-direction: column;
    padding: 18px;
    border-radius: 18px;
    background: linear-gradient(180deg, rgba(42, 39, 60, 0.98), rgba(30, 28, 42, 0.98));
  }

  header {
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }

  h2 {
    margin: 0 0 4px;
    font-family: var(--font-display);
    font-size: 18px;
    font-weight: 650;
  }

  header p {
    margin: 0 0 12px;
    font-size: 13px;
    color: var(--ink-2);
  }

  .close {
    margin-left: auto;
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 8px;
    color: var(--ink-2);
    flex: none;
  }

  .close:hover {
    background: var(--tint-2);
    color: var(--ink);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
  }

  .option {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 8px;
    border-radius: 12px;
    text-align: left;
  }

  .option:hover:not(:disabled) {
    background: var(--tint-2);
  }

  .option.in-use {
    background: var(--tint-1);
    cursor: default;
  }

  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .text strong,
  .text span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .text span {
    font-size: 12.5px;
    color: var(--ink-2);
  }

  .badge {
    padding: 3px 8px;
    border-radius: 999px;
    font-size: 11.5px;
    background: var(--violet-dim);
    color: var(--ink);
  }

  .status {
    margin: 8px 0;
    color: var(--ink-2);
  }
</style>

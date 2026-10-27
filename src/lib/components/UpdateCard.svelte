<!-- "Atualização disponível": verifica ao abrir e a cada 6 h; baixa, instala e reinicia. -->
<script lang="ts">
  import { onMount } from "svelte";
  import Download from "@lucide/svelte/icons/download";
  import { api } from "$lib/api";
  import { updates } from "$lib/updates.svelte";

  /** Espera o app assentar antes da primeira verificação: abrir rápido vem primeiro. */
  const FIRST_CHECK_MS = 5_000;
  const CHECK_EVERY_MS = 6 * 60 * 60 * 1000;

  const update = $derived(updates.update);
  const progress = $derived(updates.progress);
  const pct = $derived(progress?.total ? Math.min(100, (progress.downloaded / progress.total) * 100) : null);
  const check = () => void updates.check();

  onMount(() => {
    void api.onUpdateProgress((p) => (updates.progress = p));
    const first = setTimeout(check, FIRST_CHECK_MS);
    const every = setInterval(check, CHECK_EVERY_MS);
    return () => {
      clearTimeout(first);
      clearInterval(every);
    };
  });

  const mb = (bytes: number) => (bytes / 1024 / 1024).toFixed(1);
</script>

{#if update && !updates.dismissed}
  <aside class="update glass" role="status" aria-live="polite">
    <div class="head">
      <span class="icon"><Download size={16} strokeWidth={1.8} /></span>
      <div>
        <strong>Atualização disponível</strong>
        <span class="versions tabular">{update.currentVersion} → {update.version}</span>
      </div>
    </div>

    {#if update.notes}
      <p class="notes">{update.notes}</p>
    {/if}

    {#if updates.installing}
      <div class="bar" aria-hidden="true"><span style:width="{pct ?? 15}%" class:indeterminate={pct === null}></span></div>
      <p class="status tabular">
        {#if progress && pct !== null && pct >= 100}
          Instalando; o GlobalMusic vai reabrir sozinho…
        {:else if progress}
          Baixando {mb(progress.downloaded)}{progress.total ? ` de ${mb(progress.total)}` : ""} MB…
        {:else}
          Preparando…
        {/if}
      </p>
    {:else}
      <div class="actions">
        <button class="btn" onclick={() => updates.install()}>Atualizar agora</button>
        <button class="btn ghost" onclick={() => (updates.dismissed = true)}>Depois</button>
      </div>
    {/if}

    {#if updates.error}
      <p class="error" role="alert">{updates.error}</p>
    {/if}
  </aside>
{/if}

<style>
  .update {
    position: fixed;
    right: 24px;
    bottom: 24px;
    z-index: 10;
    width: 320px;
    padding: 16px;
    border-radius: 18px;
    /* O card flutua sobre o painel: precisa de um fundo mais sólido que o vidro comum. */
    background: linear-gradient(180deg, rgba(40, 37, 58, 0.96), rgba(28, 26, 40, 0.96));
    animation: rise 260ms ease-out;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .head div {
    display: flex;
    flex-direction: column;
  }

  .icon {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 10px;
    background: var(--violet);
    color: #fff;
  }

  .versions {
    font-size: 12px;
    color: var(--ink-3);
  }

  .notes {
    margin: 12px 0 0;
    white-space: pre-line;
    font-size: 13px;
    color: var(--ink-2);
    max-height: 7.5em;
    overflow-y: auto;
  }

  .actions {
    display: flex;
    gap: 8px;
    margin-top: 14px;
  }

  .btn {
    padding: 7px 14px;
    border-radius: 10px;
    font-weight: 600;
    background: var(--violet);
    color: #fff;
  }

  .btn:hover {
    background: var(--violet-bright);
  }

  .btn.ghost {
    background: none;
    color: var(--ink-2);
    border: 1px solid var(--edge);
  }

  .btn.ghost:hover {
    color: var(--ink);
    background: var(--tint-1);
  }

  .bar {
    margin-top: 14px;
    height: 5px;
    border-radius: 999px;
    background: var(--violet-dim);
    overflow: hidden;
  }

  .bar span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--violet-bright);
    transition: width 200ms linear;
  }

  .bar .indeterminate {
    animation: slide 1.2s ease-in-out infinite;
  }

  .status {
    margin: 8px 0 0;
    font-size: 12px;
    color: var(--ink-3);
  }

  .error {
    margin: 10px 0 0;
    font-size: 12.5px;
    color: #ffb4b4;
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(12px);
    }
  }

  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(700%);
    }
  }
</style>

<!-- Letra da faixa atual (LRCLIB). Sincronizada: o verso do momento fica destacado e centralizado;
     clicar num verso pula até ele. Sem sincronia: a letra corrida. -->
<script lang="ts">
  import { app } from "$lib/app.svelte";

  const lyrics = $derived(app.lyrics);
  const synced = $derived(lyrics?.data?.synced ?? []);

  /** Último verso que já começou (com 0,3 s de antecedência, para o destaque não atrasar). */
  const active = $derived.by(() => {
    let current = -1;
    for (let i = 0; i < synced.length && synced[i].timeMs <= app.positionMs + 300; i++) current = i;
    return current;
  });

  let box = $state<HTMLDivElement>();
  const reduceMotion = typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

  // Rola só a caixa da letra (scrollIntoView rolaria a tela inteira e esconderia os controles).
  $effect(() => {
    const line = box?.querySelector<HTMLElement>(`[data-line="${active}"]`);
    if (!box || !line) return;
    const top = line.offsetTop - box.clientHeight / 2 + line.clientHeight / 2;
    box.scrollTo({ top, behavior: reduceMotion ? "auto" : "smooth" });
  });
</script>

<div class="lyrics" bind:this={box} aria-live="off">
  {#if !lyrics || lyrics.loading}
    <p class="status">Buscando a letra…</p>
  {:else if lyrics.error}
    <p class="status">Não deu para buscar a letra agora.</p>
  {:else if !lyrics.data}
    <p class="status">Sem letra para esta música.</p>
  {:else if lyrics.data.instrumental}
    <p class="status">Instrumental</p>
  {:else if synced.length}
    {#each synced as line, i (i)}
      <button
        class="line"
        class:active={i === active}
        class:past={i < active}
        data-line={i}
        onclick={() => app.commitSeek(line.timeMs)}
      >
        {line.text || "♪"}
      </button>
    {/each}
  {:else}
    <p class="plain">{lyrics.data.plain}</p>
  {/if}
</div>

<style>
  .lyrics {
    position: relative; /* referência do offsetTop dos versos */
    flex: none;
    width: min(100%, 560px);
    height: min(40vh, 320px);
    box-sizing: border-box;
    overflow-y: auto;
    padding: 0 8px;
    text-align: center;
    scrollbar-width: none;
    mask-image: linear-gradient(transparent, #000 18%, #000 82%, transparent);
  }

  /* Espaço antes do 1º e depois do último verso, para eles também chegarem ao centro.
     (Padding em % seria calculado pela largura, e a caixa cresceria escondendo os controles.) */
  .lyrics::before,
  .lyrics::after {
    content: "";
    display: block;
    height: 45%;
  }

  .line {
    display: block;
    width: 100%;
    padding: 5px 0;
    font-family: var(--font-display);
    font-size: clamp(17px, 1.9vw, 22px);
    font-weight: 650;
    line-height: 1.3;
    color: var(--ink-3);
    transition: color 0.2s;
  }

  .line.past {
    color: var(--ink-2);
  }

  .line.active {
    color: var(--ink);
  }

  .line:hover {
    color: var(--violet-bright);
  }

  .plain {
    margin: 0;
    white-space: pre-wrap;
    font-size: 16px;
    line-height: 1.7;
    color: var(--ink-2);
  }

  .status {
    margin: 0;
    color: var(--ink-3);
  }

  @media (prefers-reduced-motion: reduce) {
    .line {
      transition: none;
    }
  }
</style>

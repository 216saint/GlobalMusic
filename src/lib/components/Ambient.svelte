<!-- Fundo ambiente: a capa atual, ampliada e desfocada — o único blur da UI.
     Sem capa, duas manchas nas cores do ícone. -->
<script lang="ts">
  import { app } from "$lib/app.svelte";

  // A miniatura basta: com 70px de blur, resolução não aparece — e custa ~25x menos memória.
  const art = $derived(app.player.current?.artwork?.thumb ?? null);
</script>

<div class="ambient" aria-hidden="true">
  {#if art}
    {#key art}
      <img src={art} alt="" />
    {/key}
  {:else}
    <div class="blobs"></div>
  {/if}
  <div class="veil"></div>
</div>

<style>
  .ambient {
    position: fixed;
    inset: 0;
    z-index: -1;
    overflow: hidden;
    border-radius: var(--r-window);
    pointer-events: none;
  }

  img {
    position: absolute;
    inset: -20%;
    width: 140%;
    height: 140%;
    object-fit: cover;
    filter: blur(70px) saturate(1.5);
    opacity: 0.7;
    animation: fade 700ms ease-out;
  }

  .blobs {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(48% 55% at 78% 22%, rgba(123, 108, 255, 0.42), transparent 70%),
      radial-gradient(40% 50% at 12% 88%, rgba(75, 63, 214, 0.38), transparent 70%),
      radial-gradient(35% 40% at 60% 95%, rgba(214, 120, 160, 0.16), transparent 70%);
  }

  /* Escurece o suficiente para o texto manter contraste sobre qualquer capa. */
  .veil {
    position: absolute;
    inset: 0;
    background: rgba(14, 13, 20, 0.38);
  }

  :global(html[data-glass="native"]) .veil {
    background: rgba(14, 13, 20, 0.5);
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }
</style>

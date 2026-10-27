<!-- Menu do botão direito sobre uma faixa: ações no GlobalMusic e na conta do serviço de origem. -->
<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import Heart from "@lucide/svelte/icons/heart";
  import HeartOff from "@lucide/svelte/icons/heart-off";
  import ListEnd from "@lucide/svelte/icons/list-end";
  import ListStart from "@lucide/svelte/icons/list-start";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import Replace from "@lucide/svelte/icons/replace";
  import { app } from "$lib/app.svelte";
  import { SOURCE_LABEL, trackUrl, type Track } from "$lib/types";

  const WIDTH = 270;
  const HEIGHT = 230;

  const menu = $derived(app.menu);
  // Mantém o menu dentro da janela mesmo clicando perto das bordas.
  const left = $derived(menu ? Math.min(menu.x, window.innerWidth - WIDTH - 8) : 0);
  const top = $derived(menu ? Math.min(menu.y, window.innerHeight - HEIGHT - 8) : 0);

  // A faixa é lida ANTES de fechar o menu e passada para a ação. (O `{@const track}` do markup é
  // recalculado a partir de `app.menu` a cada leitura: lido depois de fechar, ele quebrava a ação.)
  function run(action: (track: Track) => unknown) {
    const track = app.menu?.track;
    app.menu = null;
    if (track) void action(track);
  }
</script>

<svelte:window
  onkeydown={(e) => e.key === "Escape" && (app.menu = null)}
  onblur={() => (app.menu = null)}
/>

{#if menu}
  {@const track = menu.track}
  {@const liked = app.isFavorite(track)}
  {@const url = trackUrl(track)}
  <!-- Clique fora fecha o menu. -->
  <div class="backdrop" role="presentation" onclick={() => (app.menu = null)} oncontextmenu={(e) => { e.preventDefault(); app.menu = null; }}></div>
  <div class="menu glass" role="menu" style:left="{left}px" style:top="{top}px" style:width="{WIDTH}px">
    <p class="head">{track.title}</p>
    <button role="menuitem" onclick={() => run((t) => app.playNext(t))}>
      <ListStart size={15} /> Tocar a seguir
    </button>
    <button role="menuitem" onclick={() => run((t) => app.enqueue(t))}>
      <ListEnd size={15} /> Adicionar à fila
    </button>
    <button role="menuitem" onclick={() => run((t) => app.toggleFavorite(t))}>
      {#if liked}
        <HeartOff size={15} /> Descurtir (GlobalMusic e {SOURCE_LABEL[track.source]})
      {:else}
        <Heart size={15} /> Curtir (GlobalMusic e {SOURCE_LABEL[track.source]})
      {/if}
    </button>
    {#if track.source === "spotify"}
      <!-- Quando a música do Spotify toca pelo YouTube, a busca pode escolher a versão errada. -->
      <button role="menuitem" onclick={() => run((t) => app.openVersions(t))}>
        <Replace size={15} /> Escolher versão do YouTube
      </button>
    {/if}
    {#if url}
      <button role="menuitem" onclick={() => run((t) => { const link = trackUrl(t); if (link) return openUrl(link); })}>
        <ExternalLink size={15} /> Abrir no {SOURCE_LABEL[track.source]}
      </button>
    {/if}
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 30;
  }

  .menu {
    position: fixed;
    z-index: 31;
    padding: 6px;
    border-radius: 14px;
    background: linear-gradient(180deg, rgba(42, 39, 60, 0.98), rgba(30, 28, 42, 0.98));
  }

  .head {
    margin: 4px 10px 6px;
    font-size: 12px;
    color: var(--ink-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  button {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 10px;
    border-radius: 9px;
    font-size: 13px;
    text-align: left;
    color: var(--ink);
  }

  button:hover {
    background: var(--tint-2);
  }
</style>

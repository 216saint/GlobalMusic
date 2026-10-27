<!-- Menu do botão direito sobre uma playlist: tocar, abrir, compartilhar e, nas do GlobalMusic,
     editar e apagar. -->
<script lang="ts">
  import Play from "@lucide/svelte/icons/play";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import Share2 from "@lucide/svelte/icons/share-2";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import { app, type PlaylistTarget } from "$lib/app.svelte";
  import { SOURCE_LABEL, playlistUrl } from "$lib/types";

  const WIDTH = 250;
  const HEIGHT = 220;

  const menu = $derived(app.playlistMenu);
  const left = $derived(menu ? Math.min(menu.x, window.innerWidth - WIDTH - 8) : 0);
  const top = $derived(menu ? Math.min(menu.y, window.innerHeight - HEIGHT - 8) : 0);

  // Como no menu da faixa: o alvo é lido antes de fechar o menu.
  function run(action: (target: PlaylistTarget) => unknown) {
    const target = app.playlistMenu?.target;
    app.playlistMenu = null;
    if (target) void action(target);
  }

  let confirming = $state(false);
  $effect(() => {
    void menu;
    confirming = false;
  });
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && (app.playlistMenu = null)} onblur={() => (app.playlistMenu = null)} />

{#if menu}
  {@const target = menu.target}
  <div
    class="backdrop"
    role="presentation"
    onclick={() => (app.playlistMenu = null)}
    oncontextmenu={(e) => {
      e.preventDefault();
      app.playlistMenu = null;
    }}
  ></div>
  <div class="menu glass" role="menu" style:left="{left}px" style:top="{top}px" style:width="{WIDTH}px">
    <p class="head">{target.playlist.name}</p>
    {#if target.kind === "local"}
      <button role="menuitem" disabled={!target.playlist.tracks.length} onclick={() => run((t) => t.kind === "local" && app.play(t.playlist.tracks, 0, t.playlist.name))}>
        <Play size={15} /> Tocar
      </button>
      <button role="menuitem" onclick={() => run((t) => t.kind === "local" && app.openLocal(t.playlist))}>
        <FolderOpen size={15} /> Abrir
      </button>
      <button role="menuitem" onclick={() => run((t) => t.kind === "local" && (app.editing = t.playlist))}>
        <Pencil size={15} /> Editar nome e foto
      </button>
      <button role="menuitem" onclick={() => run((t) => t.kind === "local" && app.sharePlaylist(t.playlist))}>
        <Share2 size={15} /> Compartilhar (copiar link)
      </button>
      {#if confirming}
        <button role="menuitem" class="danger" onclick={() => run((t) => t.kind === "local" && app.deletePlaylist(t.playlist))}>
          <Trash2 size={15} /> Confirmar: apagar playlist
        </button>
      {:else}
        <button role="menuitem" onclick={() => (confirming = true)}>
          <Trash2 size={15} /> Apagar
        </button>
      {/if}
    {:else}
      {@const url = playlistUrl(target.playlist)}
      <button role="menuitem" onclick={() => run((t) => t.kind === "service" && app.playService(t.playlist))}>
        <Play size={15} /> Tocar
      </button>
      <button role="menuitem" onclick={() => run((t) => t.kind === "service" && app.openPlaylist(t.playlist))}>
        <FolderOpen size={15} /> Abrir
      </button>
      {#if url}
        <button role="menuitem" onclick={() => run(() => app.copyLink(url, `Link do ${SOURCE_LABEL[target.playlist.source]} copiado`))}>
          <Share2 size={15} /> Compartilhar (copiar link do {SOURCE_LABEL[target.playlist.source]})
        </button>
      {/if}
      {#if app.isImported(target.playlist)}
        <button role="menuitem" onclick={() => run((t) => t.kind === "service" && app.removeImported(t.playlist))}>
          <X size={15} /> Tirar da biblioteca
        </button>
      {/if}
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

  button:hover:not(:disabled) {
    background: var(--tint-2);
  }

  button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .danger,
  .danger:hover:not(:disabled) {
    background: #c42b1c;
    color: #fff;
  }
</style>

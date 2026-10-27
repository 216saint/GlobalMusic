<!-- Barra de título: logo + nome à esquerda, navegação e filtros **no centro da janela**, fila, amigos e
     perfil à direita, depois os controles da janela. Grid de 3 colunas (1fr auto 1fr): o meio fica
     centralizado de verdade, seja qual for a largura dos lados.
     O fundo é área de arrastar (data-tauri-drag-region); os botões não. -->
<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Minus from "@lucide/svelte/icons/minus";
  import Square from "@lucide/svelte/icons/square";
  import X from "@lucide/svelte/icons/x";
  import CircleUser from "@lucide/svelte/icons/circle-user";
  import Search from "@lucide/svelte/icons/search";
  import Library from "@lucide/svelte/icons/library";
  import Layers from "@lucide/svelte/icons/layers";
  import ListMusic from "@lucide/svelte/icons/list-music";
  import Users from "@lucide/svelte/icons/users";
  import { app, type Filter, type Panel, type Tab } from "$lib/app.svelte";
  import { SOURCES, SOURCE_LABEL } from "$lib/types";
  import { social } from "$lib/social.svelte";
  import Avatar from "./Avatar.svelte";
  import Bars from "./Bars.svelte";
  import SourceIcon from "./SourceIcon.svelte";

  const os = document.documentElement.dataset.os;
  // No macOS os semáforos são nativos; no navegador (demo) não há janela para controlar.
  const customControls = os === "windows" || os === "linux";
  const win = customControls ? getCurrentWindow() : null;

  const TABS: { tab: Tab; label: string; icon: typeof Search }[] = [
    { tab: "search", label: "Buscar", icon: Search },
    { tab: "library", label: "Biblioteca", icon: Library },
  ];
  const FILTERS: Filter[] = ["all", ...SOURCES];

  const togglePanel = (panel: Panel) => (app.panel = app.panel === panel ? "now" : panel);
</script>

<header class="titlebar" class:mac={os === "macos"} data-tauri-drag-region>
  <div class="brand" data-tauri-drag-region>
    <!-- Só as barras do ícone, sem o fundo escuro do PNG. -->
    <span class="logo" data-tauri-drag-region><Bars size={20} /></span>
    <span class="name" data-tauri-drag-region>GlobalMusic</span>
  </div>

  <nav class="nav" aria-label="Navegação">
    <div class="group" role="tablist">
      {#each TABS as { tab, label, icon: Icon }}
        <button
          class="icon-btn"
          class:on={app.sidebarOpen && app.tab === tab}
          role="tab"
          aria-selected={app.sidebarOpen && app.tab === tab}
          aria-label={label}
          title={label}
          onclick={() => app.setTab(tab)}
        >
          <Icon size={17} strokeWidth={1.7} />
        </button>
      {/each}
    </div>

    <span class="divider" aria-hidden="true"></span>

    <div class="group" role="radiogroup" aria-label="Fonte">
      {#each FILTERS as f}
        {@const label = f === "all" ? "Todas as fontes" : SOURCE_LABEL[f]}
        <button
          class="icon-btn"
          class:on={app.filter === f}
          role="radio"
          aria-checked={app.filter === f}
          aria-label={label}
          title={label}
          onclick={() => app.setFilter(f)}
        >
          {#if f === "all"}
            <Layers size={16} strokeWidth={1.7} />
          {:else}
            <SourceIcon source={f} size={16} mono={app.filter !== f} />
          {/if}
        </button>
      {/each}
    </div>
  </nav>

  <div class="actions" data-tauri-drag-region>
    <button
      class="icon-btn"
      class:on={app.panel === "queue"}
      aria-label="Fila"
      title="Fila"
      aria-pressed={app.panel === "queue"}
      onclick={() => togglePanel("queue")}
    >
      <ListMusic size={18} strokeWidth={1.6} />
    </button>
    <button
      class="icon-btn"
      class:on={app.panel === "friends"}
      aria-label={social.pendingCount ? `Amigos (${social.pendingCount} pedido de amizade)` : "Amigos"}
      title="Amigos"
      aria-pressed={app.panel === "friends"}
      onclick={() => togglePanel("friends")}
    >
      <Users size={18} strokeWidth={1.6} />
      {#if social.pendingCount}
        <span class="badge" aria-hidden="true"></span>
      {/if}
    </button>
    <button
      class="icon-btn profile"
      class:on={app.panel === "profile"}
      aria-label="Perfil e contas"
      title="Perfil e contas"
      aria-pressed={app.panel === "profile"}
      onclick={() => togglePanel("profile")}
    >
      {#if social.me}
        <Avatar name={social.me.displayName} url={social.me.avatarUrl} size={22} />
      {:else}
        <CircleUser size={18} strokeWidth={1.6} />
      {/if}
    </button>

    {#if win}
      <div class="window-controls">
        <button aria-label="Minimizar" onclick={() => win.minimize()}><Minus size={16} strokeWidth={1.4} /></button>
        <button aria-label="Maximizar" onclick={() => win.toggleMaximize()}><Square size={13} strokeWidth={1.4} /></button>
        <button class="close" aria-label="Fechar" onclick={() => win.close()}><X size={17} strokeWidth={1.4} /></button>
      </div>
    {/if}
  </div>
</header>

<style>
  .titlebar {
    height: 44px;
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: 16px;
    padding-left: 16px;
  }

  /* Espaço para os semáforos nativos. */
  .titlebar.mac {
    padding-left: 84px;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .logo {
    display: grid;
    place-items: center;
    width: 20px;
    pointer-events: none; /* o arrasto da janela passa para a barra */
  }

  .name {
    font-size: 13px;
    font-weight: 650;
    color: var(--ink-2);
    letter-spacing: 0.01em;
  }

  .nav {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .group {
    display: flex;
    gap: 2px;
  }

  .divider {
    width: 1px;
    height: 18px;
    background: var(--edge);
  }

  .actions {
    display: flex;
    align-items: center;
    justify-self: end;
    height: 100%;
    gap: 2px;
  }

  .icon-btn {
    position: relative;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 10px;
    color: var(--ink-3);
  }

  .icon-btn:hover {
    background: var(--tint-1);
    color: var(--ink);
  }

  .icon-btn.on {
    background: var(--tint-3);
    color: var(--ink);
    box-shadow: inset 0 1px 0 var(--edge-top);
  }

  .icon-btn.profile {
    margin-right: 8px;
  }

  /* Pedido de amizade esperando resposta. */
  .badge {
    position: absolute;
    top: 4px;
    right: 4px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--violet-bright);
    box-shadow: 0 0 0 2px var(--graphite);
  }

  .window-controls {
    display: flex;
    height: 100%;
  }

  .window-controls button {
    width: 46px;
    height: 100%;
    display: grid;
    place-items: center;
    color: var(--ink-2);
  }

  .window-controls button:hover {
    background: var(--tint-2);
    color: var(--ink);
  }

  .window-controls .close:hover {
    background: #c42b1c;
    color: #fff;
  }

  .window-controls button:last-child {
    border-top-right-radius: var(--r-window);
  }
</style>

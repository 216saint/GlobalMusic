<!-- Aba Amigos: busca por @usuário, pedidos e a lista. Precisa da conta GlobalMusic (no Perfil). -->
<script lang="ts">
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import { app } from "$lib/app.svelte";
  import { social } from "$lib/social.svelte";
  import Friends from "./Friends.svelte";
</script>

<section class="panel" aria-label="Amigos">
  <button class="back" onclick={() => (app.panel = "now")}><ChevronLeft size={16} /> Voltar ao player</button>
  <h1>Amigos</h1>

  {#if !social.ready}
    <p class="intro">Carregando…</p>
  {:else if social.me}
    <p class="intro">Seu @ é <strong>@{social.me.username}</strong>. Passe para quem quiser te adicionar.</p>
    <Friends />
  {:else}
    <p class="intro">Para adicionar amigos, entre na sua conta GlobalMusic (com Discord ou Spotify).</p>
    <button class="btn" onclick={() => (app.panel = "profile")}>Entrar pelo Perfil</button>
  {/if}
</section>

<style>
  .panel {
    padding: 28px 40px;
    overflow-y: auto;
  }

  .back {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--ink-2);
    font-size: 13px;
  }

  .back:hover {
    color: var(--ink);
  }

  h1 {
    margin: 28px 0 6px;
    font-family: var(--font-display);
    font-size: 28px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }

  .intro {
    margin: 0 0 20px;
    color: var(--ink-2);
    max-width: 56ch;
  }

  .intro strong {
    color: var(--ink);
    user-select: all;
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
</style>

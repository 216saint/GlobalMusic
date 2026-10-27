<!-- Conteúdo da aba Amigos: buscar por @usuário, pedidos recebidos/enviados e a lista de amigos. -->
<script lang="ts">
  import Search from "@lucide/svelte/icons/search";
  import { social } from "$lib/social.svelte";
  import type { PublicUser } from "$lib/types";
  import Avatar from "./Avatar.svelte";

  let query = $state("");
  let timer: ReturnType<typeof setTimeout> | undefined;

  // Espera a pessoa parar de digitar para não fazer uma busca por tecla.
  function onInput() {
    clearTimeout(timer);
    timer = setTimeout(() => void social.search(query), 250);
  }
</script>

{#snippet person(user: PublicUser)}
  <Avatar name={user.displayName} url={user.avatarUrl} size={34} />
  <div class="text">
    <strong>{user.displayName}</strong>
    <span>@{user.username}</span>
  </div>
{/snippet}

<section class="friends" aria-label="Amigos">
  <label class="search">
    <Search size={15} />
    <span class="sr-only">Buscar pessoas por @usuário</span>
    <input placeholder="Buscar por @usuário" bind:value={query} oninput={onInput} spellcheck="false" autocapitalize="off" />
  </label>

  {#if query.trim().replace(/^@/, "").length >= 2}
    <ul class="list">
      {#each social.results as user (user.username)}
        {@const relation = social.relation(user.username)}
        <li>
          {@render person(user)}
          {#if relation === "friend"}
            <span class="state">Amigos</span>
          {:else if relation === "outgoing"}
            <span class="state">Pedido enviado</span>
          {:else if relation === "incoming"}
            <button class="btn" onclick={() => social.addFriend(user.username)}>Aceitar</button>
          {:else}
            <button class="btn" onclick={() => social.addFriend(user.username)}>Adicionar</button>
          {/if}
        </li>
      {:else}
        <li class="empty">{social.searching ? "Buscando…" : "Ninguém com esse @usuário."}</li>
      {/each}
    </ul>
  {/if}

  {#if social.friends.incoming.length}
    <h3>Pedidos para você</h3>
    <ul class="list">
      {#each social.friends.incoming as user (user.username)}
        <li>
          {@render person(user)}
          <button class="btn" onclick={() => social.addFriend(user.username)}>Aceitar</button>
          <button class="btn ghost" onclick={() => social.removeFriend(user.username)}>Recusar</button>
        </li>
      {/each}
    </ul>
  {/if}

  <h3>Seus amigos</h3>
  <ul class="list">
    {#each social.friends.friends as user (user.username)}
      <li>
        {@render person(user)}
        <button class="btn ghost" onclick={() => social.removeFriend(user.username)}>Desfazer amizade</button>
      </li>
    {:else}
      <li class="empty">Busque alguém pelo @usuário para mandar um pedido.</li>
    {/each}
  </ul>

  {#if social.friends.outgoing.length}
    <h3>Pedidos enviados</h3>
    <ul class="list">
      {#each social.friends.outgoing as user (user.username)}
        <li>
          {@render person(user)}
          <button class="btn ghost" onclick={() => social.removeFriend(user.username)}>Cancelar pedido</button>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .friends {
    max-width: 640px;
  }

  h3 {
    margin: 20px 0 8px;
    font-size: 13px;
    font-weight: 600;
    color: var(--ink-2);
  }

  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    border-radius: 12px;
    border: 1px solid var(--edge);
    background: var(--tint-1);
    color: var(--ink-3);
  }

  .search:focus-within {
    border-color: var(--violet);
  }

  .search input {
    flex: 1;
    min-width: 0;
    padding: 9px 0;
    border: none;
    background: none;
    outline: none;
    color: var(--ink);
  }

  .list {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }

  .list li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 10px;
    border-radius: 14px;
    background: var(--tint-1);
    border: 1px solid var(--edge);
  }

  .list li.empty {
    color: var(--ink-2);
    font-size: 13px;
    background: none;
    border-style: dashed;
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

  .state {
    font-size: 12.5px;
    color: var(--ink-3);
  }

  .btn {
    padding: 6px 12px;
    border-radius: 10px;
    font-weight: 600;
    font-size: 13px;
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
</style>

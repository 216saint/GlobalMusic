<!-- Foto de perfil redonda; sem foto (ou se ela não carregar), a inicial do nome. -->
<script lang="ts">
  let { name, url, size = 36 }: { name: string; url: string | null; size?: number } = $props();

  let broken = $state(false);
  $effect(() => {
    void url;
    broken = false;
  });
  const initial = $derived((name.trim()[0] ?? "?").toUpperCase());
</script>

<span class="avatar" style:--size="{size}px" aria-hidden="true">
  {#if url && !broken}
    <img src={url} alt="" referrerpolicy="no-referrer" onerror={() => (broken = true)} />
  {:else}
    {initial}
  {/if}
</span>

<style>
  .avatar {
    flex: none;
    display: grid;
    place-items: center;
    width: var(--size);
    height: var(--size);
    border-radius: 50%;
    overflow: hidden;
    background: var(--violet-dim);
    border: 1px solid var(--edge);
    font-family: var(--font-display);
    font-weight: 650;
    font-size: calc(var(--size) * 0.42);
    color: #fff;
  }

  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
</style>

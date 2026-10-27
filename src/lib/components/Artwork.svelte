<!-- Capa com fallback: sem imagem, um degradê estável derivado do id. -->
<script lang="ts">
  let { url, seed, size, radius = "var(--r-row)", alt = "" }: {
    url: string | null;
    seed: string;
    size: string;
    radius?: string;
    alt?: string;
  } = $props();

  let failed = $state(false);
  $effect(() => {
    void url;
    failed = false;
  });

  const hue = $derived([...seed].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 360, 7));
</script>

<div
  class="art"
  style:width={size}
  style:height={size}
  style:border-radius={radius}
  style:--h={hue}
>
  {#if url && !failed}
    <img src={url} {alt} loading="lazy" decoding="async" onerror={() => (failed = true)} />
  {/if}
</div>

<style>
  .art {
    position: relative;
    flex: none;
    overflow: hidden;
    background:
      radial-gradient(120% 90% at 20% 15%, hsl(var(--h) 70% 62% / 0.95), transparent 60%),
      radial-gradient(120% 120% at 90% 100%, hsl(calc(var(--h) + 70) 65% 45%), hsl(calc(var(--h) + 140) 40% 18%));
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08);
  }

  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
</style>

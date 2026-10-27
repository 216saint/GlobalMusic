<!-- Capa das "Curtidas" de cada serviço: degradê na cor dele + coração (como a do Spotify),
     no lugar de uma capa genérica ou da foto de uma música qualquer. -->
<script lang="ts">
  import Heart from "@lucide/svelte/icons/heart";
  import type { Source } from "$lib/types";

  let { source, size = "100%", radius = "14px" }: { source: Source; size?: string; radius?: string } = $props();
</script>

<div class="liked {source}" style:width={size} style:height={size} style:border-radius={radius} aria-hidden="true">
  <Heart size={48} fill="currentColor" strokeWidth={0} />
</div>

<style>
  .liked {
    flex: none;
    display: grid;
    place-items: center;
    color: #fff;
    container-type: size;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.12);
  }

  /* O coração acompanha o tamanho da capa (card grande ou miniatura). */
  .liked :global(svg) {
    width: 38cqmin;
    height: 38cqmin;
    filter: drop-shadow(0 4px 12px rgba(0, 0, 0, 0.3));
  }

  .spotify {
    background: linear-gradient(135deg, #1ed760 0%, #128a3e 55%, #0a3d1d 100%);
  }

  .youtube_music {
    background: linear-gradient(135deg, #ff4e64 0%, #d0102b 55%, #5c0612 100%);
  }

  .soundcloud {
    background: linear-gradient(135deg, #ff8a3d 0%, #ff5500 55%, #7a2400 100%);
  }
</style>

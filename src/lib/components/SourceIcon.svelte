<script lang="ts">
  import type { Source } from "$lib/types";
  import { SOURCE_LABEL } from "$lib/types";

  let { source, size = 16, mono = false }: { source: Source; size?: number; mono?: boolean } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  role="img"
  aria-label={SOURCE_LABEL[source]}
  style:color={mono ? "currentColor" : `var(--${source})`}
>
  {#if source === "spotify"}
    <circle cx="12" cy="12" r="11" fill="currentColor" />
    <g fill="none" stroke="#0b0b0b" stroke-linecap="round">
      <path d="M6.2 9.2c3.9-1.2 8.4-.9 11.8 1" stroke-width="1.9" />
      <path d="M6.9 12.5c3.2-.9 6.8-.6 9.6.9" stroke-width="1.6" />
      <path d="M7.5 15.6c2.5-.6 5.2-.4 7.4.8" stroke-width="1.35" />
    </g>
  {:else if source === "youtube_music"}
    <circle cx="12" cy="12" r="11" fill="currentColor" />
    <circle cx="12" cy="12" r="5.6" fill="none" stroke="#fff" stroke-width="1.3" />
    <path d="M10.5 9.6v4.8l4-2.4z" fill="#fff" />
  {:else}
    <path
      fill="currentColor"
      d="M19.4 11.1a4 4 0 0 0-1.6.3A5.6 5.6 0 0 0 12.3 6c-.6 0-1.2.1-1.7.3-.2.1-.3.2-.3.4v10.6c0 .2.2.4.4.4h8.7a3.3 3.3 0 0 0 0-6.6zM8.9 7.6c-.2 0-.4.2-.4.4v9.3c0 .2.2.4.4.4s.4-.2.4-.4V8c0-.2-.2-.4-.4-.4zM7.1 9.3c-.2 0-.4.2-.4.4v7.6c0 .2.2.4.4.4s.4-.2.4-.4V9.7c0-.2-.2-.4-.4-.4zM5.3 10.2c-.2 0-.4.2-.4.4v6.7c0 .2.2.4.4.4s.4-.2.4-.4v-6.7c0-.2-.2-.4-.4-.4zM3.5 11.6c-.2 0-.4.2-.4.4v5c0 .2.2.4.4.4s.4-.2.4-.4v-5c0-.2-.2-.4-.4-.4zM1.7 13.3c-.2 0-.4.2-.4.4v2.6c0 .2.2.4.4.4s.4-.2.4-.4v-2.6c0-.2-.2-.4-.4-.4z"
    />
  {/if}
</svg>

<style>
  svg {
    display: block;
    flex: none;
  }
</style>

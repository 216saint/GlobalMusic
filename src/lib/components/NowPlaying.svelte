<script lang="ts">
  import SkipBack from "@lucide/svelte/icons/skip-back";
  import SkipForward from "@lucide/svelte/icons/skip-forward";
  import Play from "@lucide/svelte/icons/play";
  import Pause from "@lucide/svelte/icons/pause";
  import Volume1 from "@lucide/svelte/icons/volume-1";
  import MicVocal from "@lucide/svelte/icons/mic-vocal";
  import { api } from "$lib/api";
  import { app } from "$lib/app.svelte";
  import { SOURCE_LABEL } from "$lib/types";
  import Artwork from "./Artwork.svelte";
  import Bars from "./Bars.svelte";
  import LyricsView from "./LyricsView.svelte";
  import PillSlider from "./PillSlider.svelte";
  import SourceIcon from "./SourceIcon.svelte";

  const track = $derived(app.player.current);

  function time(ms: number) {
    const s = Math.floor(ms / 1000);
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  }
</script>

<section class="now" aria-label="Tocando agora">
  {#if track}
    {#if app.showLyrics}
      <LyricsView />
    {:else}
      <Artwork url={track.artwork?.full ?? null} seed={track.id} size="min(40vh, 320px)" radius="var(--r-art)" alt="Capa de {track.title}" />
    {/if}

    <div class="meta">
      <h1>{track.title}</h1>
      <p class="artist">{track.artists.join(", ")}</p>
      <p class="source"><SourceIcon source={track.source} size={13} /> {SOURCE_LABEL[track.source]}</p>
    </div>

    <div class="seek">
      <PillSlider
        value={app.positionMs}
        max={track.durationMs}
        label="Posição"
        onchange={(v) => app.previewSeek(v)}
        oncommit={(v) => app.commitSeek(v)}
      />
      <div class="times tabular">
        <span>{app.player.loading ? "Carregando…" : time(app.positionMs)}</span>
        <span>{time(track.durationMs)}</span>
      </div>
    </div>

    <div class="controls">
      <button aria-label="Anterior" onclick={() => api.previous()}><SkipBack size={24} strokeWidth={1.5} /></button>
      <button class="main" aria-label={app.player.playing ? "Pausar" : "Tocar"} onclick={() => api.toggle()}>
        {#if app.player.playing}
          <Pause size={34} strokeWidth={1.4} />
        {:else}
          <Play size={34} strokeWidth={1.4} />
        {/if}
      </button>
      <button aria-label="Próxima" onclick={() => api.next()}><SkipForward size={24} strokeWidth={1.5} /></button>
    </div>

    <div class="volume">
      <Volume1 size={16} strokeWidth={1.6} />
      <PillSlider value={app.player.volume} max={1} label="Volume" thick={3} onchange={(v) => api.setVolume(v)} />
      <button
        class="lyrics-toggle"
        class:on={app.showLyrics}
        aria-pressed={app.showLyrics}
        aria-label="Letra"
        title="Letra"
        onclick={() => app.toggleLyrics()}
      >
        <MicVocal size={16} strokeWidth={1.6} />
      </button>
    </div>
  {:else}
    <div class="idle">
      <Bars size={180} />
      <h1>Nada tocando ainda</h1>
      <p>Escolha uma música na busca ou abra uma playlist da sua biblioteca.</p>
    </div>
  {/if}
</section>

<style>
  .now {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 22px;
    padding: 32px 40px;
    min-height: 0;
    overflow: hidden;
  }

  .now > :global(.art) {
    box-shadow:
      0 30px 70px -24px rgba(0, 0, 0, 0.65),
      inset 0 0 0 1px rgba(255, 255, 255, 0.1);
  }

  .meta {
    text-align: center;
    max-width: 52ch;
  }

  h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: clamp(22px, 2.6vw, 30px);
    font-weight: 650;
    letter-spacing: -0.01em;
    line-height: 1.15;
  }

  .artist {
    margin: 4px 0 0;
    font-size: 15px;
    color: var(--ink-2);
  }

  .source {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin: 10px 0 0;
    font-size: 12px;
    color: var(--ink-3);
  }

  .seek {
    width: min(100%, 420px);
  }

  .lyrics-toggle {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    margin-left: 6px;
    border-radius: 9px;
    color: var(--ink-3);
  }

  .lyrics-toggle:hover {
    color: var(--ink);
    background: var(--tint-1);
  }

  .lyrics-toggle.on {
    color: var(--ink);
    background: var(--tint-3);
  }

  .times {
    display: flex;
    justify-content: space-between;
    margin-top: 2px;
    font-size: 11.5px;
    color: var(--ink-3);
  }

  .controls {
    display: flex;
    align-items: center;
    gap: 36px;
  }

  .controls button {
    display: grid;
    place-items: center;
    color: var(--ink-2);
    border-radius: 50%;
    padding: 6px;
  }

  .controls button:hover {
    color: var(--ink);
  }

  .controls .main {
    color: var(--ink);
  }

  .volume {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 160px;
    color: var(--ink-3);
  }

  .idle {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    color: var(--ink-2);
  }

  .idle h1 {
    margin: 36px 0 8px;
    font-size: 22px;
    color: var(--ink);
  }

  .idle p {
    margin: 0;
    max-width: 36ch;
  }
</style>

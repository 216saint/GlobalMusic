<!-- As barras escalonadas do ícone do GlobalMusic: trecho aceso + cauda apagada.
     Usadas como estado vazio e como indicador "tocando agora" (anima só quando toca). -->
<script lang="ts">
  type Row = [start: number, lit: number, end: number];

  const ICON_ROWS: Row[] = [
    [0, 62, 80],
    [30, 82, 100],
    [16, 60, 78],
    [0, 54, 72],
    [16, 62, 80],
    [48, 74, 100],
  ];

  let { rows = ICON_ROWS, active = false, size = 120 }: { rows?: Row[]; active?: boolean; size?: number } = $props();

  const ROW_H = 10;
  const GAP = 6;
  const height = $derived(rows.length * ROW_H + (rows.length - 1) * GAP);
</script>

<svg
  class:active
  width={size}
  height={(size * height) / 100}
  viewBox="0 0 100 {height}"
  aria-hidden="true"
>
  {#each rows as [start, lit, end], i}
    {@const y = i * (ROW_H + GAP)}
    <rect class="tail" x={start} {y} width={end - start} height={ROW_H} rx={ROW_H / 2} />
    <rect
      class="lit"
      x={start}
      {y}
      width={lit - start}
      height={ROW_H}
      rx={ROW_H / 2}
      style:animation-delay="{i * -0.37}s"
      style:transform-origin="{start}px 0"
    />
  {/each}
</svg>

<style>
  svg {
    display: block;
    overflow: visible;
  }

  .tail {
    fill: var(--violet-dim);
  }

  .lit {
    fill: var(--violet);
  }

  .active .lit {
    animation: breathe 1.6s ease-in-out infinite alternate;
  }

  @keyframes breathe {
    from {
      transform: scaleX(0.55);
    }
    to {
      transform: scaleX(1);
    }
  }
</style>

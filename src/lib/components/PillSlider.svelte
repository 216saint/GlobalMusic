<!-- Slider no formato das barras do ícone: trecho percorrido aceso, resto apagado. -->
<script lang="ts">
  let { value, max, label, onchange, oncommit, thick = 4 }: {
    value: number;
    max: number;
    label: string;
    /** A cada movimento (arrastando). */
    onchange: (value: number) => void;
    /** Quando solta: bom para ações caras, como avisar o backend. */
    oncommit?: (value: number) => void;
    thick?: number;
  } = $props();

  const pct = $derived(max > 0 ? Math.min(100, (value / max) * 100) : 0);
</script>

<input
  type="range"
  min="0"
  {max}
  step="any"
  {value}
  aria-label={label}
  style:--pct="{pct}%"
  style:--thick="{thick}px"
  oninput={(e) => onchange(Number(e.currentTarget.value))}
  onchange={(e) => oncommit?.(Number(e.currentTarget.value))}
/>

<style>
  input {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 16px;
    background: transparent;
    cursor: pointer;
  }

  input::-webkit-slider-runnable-track {
    height: var(--thick);
    border-radius: 999px;
    background: linear-gradient(90deg, var(--violet-bright) var(--pct), var(--violet-dim) var(--pct));
  }

  input::-moz-range-track {
    height: var(--thick);
    border-radius: 999px;
    background: linear-gradient(90deg, var(--violet-bright) var(--pct), var(--violet-dim) var(--pct));
  }

  input::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 13px;
    height: 13px;
    margin-top: calc((var(--thick) - 13px) / 2);
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 6px rgba(0, 0, 0, 0.4);
    transition: transform 120ms ease;
  }

  input::-moz-range-thumb {
    width: 13px;
    height: 13px;
    border: 0;
    border-radius: 50%;
    background: #fff;
  }

  input:hover::-webkit-slider-thumb {
    transform: scale(1.15);
  }
</style>

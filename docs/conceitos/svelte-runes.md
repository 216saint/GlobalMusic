# Svelte 5: runes

As *runes* são as funções com `$` que dizem ao compilador do Svelte o que é reativo.

## As que usamos

```ts
// src/lib/app.svelte.ts
class AppStore {
  query = $state("");                       // estado: mudou → a UI que o usa redesenha
  results = $state<SourceResults<Track>[]>([]);

  get tracks() {                            // getter lendo $state = valor derivado
    return interleave(this.results.map((r) => r.items));
  }
}
export const app = new AppStore();          // uma instância, importada por todos
```

```svelte
<!-- componente -->
<script lang="ts">
  let { track, onplay }: Props = $props();          // propriedades vindas do pai
  const hue = $derived(hash(track.id) % 360);       // recalcula quando track.id muda
  $effect(() => { void url; failed = false; });     // efeito colateral quando `url` muda
</script>
```

| Rune | Para quê | Exemplo no projeto |
|---|---|---|
| `$state` | valor que muda e redesenha a UI | `app.query`, `app.player` |
| `$derived` | valor calculado a partir de outros | `routeLabel` no `Browser.svelte` |
| `$props` | receber dados do componente pai | `TrackRow`, `PillSlider` |
| `$effect` | rodar código quando algo muda (use pouco!) | reset do erro de imagem no `Artwork` |

## Por que um arquivo `.svelte.ts`?

Runes só funcionam em arquivos que o compilador do Svelte processa. A extensão
`.svelte.ts` permite usar `$state` numa classe TypeScript comum, fora de componentes. Isso
cria um **store global** sem nenhuma biblioteca extra (sem Redux, sem Pinia).

## `$state.snapshot`

```ts
api.play($state.snapshot(tracks), start)
```

O `$state` embrulha objetos num *Proxy* para detectar mudanças. Antes de mandar dados para
o Rust (que serializa em JSON), tiramos uma "foto" do objeto puro com `$state.snapshot`.

# 0004 — Glassmorphism com um único desfoque

**Status:** aceita (2026-10-26)

## Contexto

O glassmorphism clássico em CSS aplica `backdrop-filter: blur(...)` em cada painel. Cada
blur obriga a GPU a ler o que está atrás, desfocar e compor de novo, a cada quadro em que
algo muda. Com vários painéis, o custo de GPU e de memória de texturas se multiplica, e a
meta é de 150 MB.

## Decisão

O desfoque acontece **uma vez só**, em uma destas camadas:

| Sistema | Quem desfoca |
|---|---|
| Windows 10/11 | O próprio Windows (**Acrylic**), aplicado à janela inteira em `window.rs` |
| macOS | O próprio macOS (**Vibrancy**) |
| Linux / navegador | `Ambient.svelte`: a capa atual ampliada com `filter: blur(70px)`, renderizada uma vez |

Os painéis (`.glass` em `app.css`) **só tingem**: um degradê branco translúcido, uma borda
mais clara no topo (o "reflexo" do vidro) e sombra. Nenhum `backdrop-filter`.

## Por que funciona visualmente

O efeito de vidro vem de **ver algo desfocado através de uma superfície translúcida**. Se
o fundo já está desfocado (pelo SO ou pela capa), o painel não precisa desfocar de novo;
basta ser translúcido.

## Detalhes de implementação

- `index.html` e `body` têm fundo **transparente**, porque senão o Acrylic do Windows ficaria escondido.
- O `main.ts` marca `<html data-glass="native|drawn">`. No Linux, o `body` ganha cor de
  fundo (grafite), já que lá não há vidro do sistema.
- O véu escuro (`.veil` no `Ambient`) garante contraste do texto sobre qualquer capa,
  inclusive capas brancas.

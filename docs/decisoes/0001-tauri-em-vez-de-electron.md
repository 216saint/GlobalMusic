# 0001 — Tauri + Svelte em vez de Electron

**Status:** aceita (2026-10-26)

## Contexto

O requisito mandatório é usar **menos de 150 MB de RAM**, somando todos os processos.
O app do Spotify é feito com CEF, uma variante do Electron, e passa de 600 MB.

## Alternativas

| Opção | RAM típica | Por que não (ou por que sim) |
|---|---|---|
| Electron | 150–300 MB vazio | Embute um Chromium inteiro por app. Estoura a meta antes de ter qualquer feature |
| Rust + egui (como o Spotifast) | 100–250 MB | Sem navegador, mas glassmorphism e layout rico dão muito mais trabalho; e o Spotifast mostra que isso não garante pouca RAM |
| **Tauri 2** | ~100 MB vazio (medido: 106,5 MB) | Usa a WebView **do sistema** (WebView2 no Windows, WebKit no macOS/Linux), que já está instalada e é compartilhada |

Para o frontend, escolhemos **Svelte 5** em vez de React. O Svelte é um *compilador*:
o componente vira JS direto, sem um runtime de Virtual DOM. O nosso bundle inteiro tem
32 KB gzip.

## Consequências

- ✅ Binário de 3,5 MB e 106,5 MB de RAM com o app aberto (medido com `npm run measure:ram`).
- ✅ Backend em Rust, que é rápido, sem garbage collector e com segurança de memória.
- ⚠️ A WebView varia por sistema: WebView2/Chromium no Windows e WebKit no macOS e no
  Linux. CSS e APIs novas precisam funcionar nos dois motores.
- ⚠️ A RAM medida inclui os processos da WebView2. Cada feature visual pesada (imagens
  grandes, `backdrop-filter`, animações) aparece direto nessa conta. Ver ADRs 0004 e 0005.

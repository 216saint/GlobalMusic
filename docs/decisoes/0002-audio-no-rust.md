# 0002 — Todo o áudio é tocado no Rust

**Status:** aceita (2026-10-26). A implementação é a fase 3.

## Contexto

Existem duas formas de tocar música num app Tauri:

1. **Na WebView**, com um `<audio src="...">`, como um site faria.
2. **No processo Rust**, decodificando e mandando o som direto para a placa de áudio.

## Decisão

Tocar tudo no Rust.

## Por quê

- **RAM e CPU:** o pipeline de mídia do Chromium/WebKit é pesado (buffers, decodificadores,
  processo de áudio separado). No Rust, `rodio` + `symphonia` decodificam com poucos MB.
- **Uma fila só:** o Spotify Premium *precisa* tocar pelo `librespot`, que é Rust. Se o
  YouTube tocasse na WebView, teríamos dois players para sincronizar (volume, fim de faixa,
  próxima). Com tudo no Rust, há um ponto único de controle.
- **Segue tocando** mesmo se a interface travar ou estiver minimizada.

## Como vai funcionar (fase 3)

```
Spotify Premium:   librespot (Session + Player) ───────────────────────────► saída de áudio
YouTube / SC:      yt-dlp -f bestaudio[ext=m4a] -o -  →  stream-download  →  rodio::Decoder ─► saída
```

- O `stream-download` permite começar a tocar **enquanto baixa** (com a feature `process`,
  ele lê direto do `stdout` do yt-dlp).
- **Por que m4a?** O `symphonia` decodifica AAC, mas não Opus, que é o outro formato que o
  YouTube oferece. Pedindo m4a, evitamos precisar do ffmpeg.
- O `rodio` 0.21 é a **mesma versão** que o `librespot` usa, então existe uma única pilha de áudio no binário.

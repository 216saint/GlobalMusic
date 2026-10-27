# Arquitetura

## O mapa

```
┌──────────────────────── processo da WebView (msedgewebview2 / WebKit) ───────────────────────┐
│  Svelte 5 (src/)                                                                             │
│   App.svelte ─ Browser / NowPlaying / Profile          estado: lib/app.svelte.ts (runes)     │
│                       │ chama                                    ▲ recebe                    │
│                       ▼                                          │                           │
│                 lib/api.ts ── invoke("search", {query}) ──┐      │ evento "player://state"   │
└───────────────────────────────────────────────────────────┼──────┼───────────────────────────┘
                                                IPC do Tauri│      │
┌──────────────────────── processo do app (Rust, src-tauri/) ▼──────┴──────────────────────────┐
│  commands.rs  ── a "API" que o frontend enxerga                                              │
│     │                                                                                        │
│     ├── aggregate.rs ── dispara a mesma operação nos 3 provedores em paralelo                │
│     │        │                                                                               │
│     │        ▼                                                                               │
│     │   providers/ ── trait MusicProvider                                                    │
│     │     ├── spotify/          → Web API (OAuth PKCE, 429, client ID)┐                          │
│     │     ├── youtube_music.rs  → InnerTube (cookies + SAPISIDHASH)├── internet              │
│     │     └── soundcloud.rs     → api-v2 (client_id público + cookie)┘                       │
│     │                                                                                        │
│     ├── player/ ── fila única + roteamento (Premium / Free→YouTube / stream direto)          │
│     ├── presence.rs ── thread própria ── IPC local ──► app do Discord                        │
│     └── auth/ ── PKCE, loopback, janela de login, cofre de tokens do SO                      │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

**Regra principal:** a interface só desenha. Toda a lógica, a rede e o áudio ficam no
Rust. Assim a WebView, que é a parte mais pesada em RAM, faz o mínimo possível.

## Caminho de uma busca

1. Você digita no campo de busca. O `Browser.svelte` espera 280 ms sem digitação
   (*debounce*) para não disparar uma busca a cada tecla.
2. `app.search(query)` chama `api.search`, que faz `invoke("search", { query, limit })`.
3. O Tauri entrega isso ao `commands::search` no Rust.
4. `aggregate::search` chama `search_tracks` **nos três provedores ao mesmo tempo**,
   cada um com um limite de 8 s (ver [async-fan-out](conceitos/async-fan-out.md)).
5. Volta um `Vec<SourceResults<Track>>`: para cada fonte, os itens **ou** o erro.
6. O frontend intercala os resultados (A1, B1, C1, A2…) para "Todas" não virar três
   blocos separados, e mostra uma linha explicando cada fonte que falhou.

Repare no `#searchSeq` em `app.svelte.ts`: se você digita "xav", depois "xavier", e a
resposta de "xav" chega por último, ela é descartada. Esse bug se chama *race condition*
e é clássico em campos de busca.

## Caminho de um "play"

1. Você clica numa faixa, e `app.play(lista, índice)` chama `player_play`.
2. `Player::play_queue` troca a fila. Para cada faixa, `routing::route_for` decide
   **como** tocar:
   - Spotify + Premium → `librespot`
   - Spotify + Free → `resolver` acha a mesma gravação no YouTube
   - YouTube Music / SoundCloud → stream direto
3. `update_player` emite o evento `player://state` para a UI e avisa o Discord.

## Onde mexer para…

| Quero… | Arquivo |
|---|---|
| adicionar uma fonte nova | crie `providers/x.rs` implementando `MusicProvider` e registre em `providers::all` |
| expor algo novo para a UI | um `#[tauri::command]` em `commands.rs` + um wrapper em `src/lib/api.ts` |
| mudar o visual | tokens em `src/app.css`, componentes em `src/lib/components/` |
| mudar quanto tempo a busca espera cada fonte | `SEARCH_TIMEOUT` em `aggregate.rs` |

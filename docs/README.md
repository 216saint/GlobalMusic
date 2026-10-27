# Documentação de estudo do GlobalMusic

Esta pasta explica **o que** foi construído, **como** e, principalmente, **por quê**.
O objetivo do projeto é aprender. Por isso, toda decisão importante tem as alternativas
que foram descartadas e o motivo do descarte.

## Por onde começar

0. [conceitos/fluxo-do-app.md](conceitos/fluxo-do-app.md): **comece aqui se você vem de web
   apps**. O que torna isto um app, quais etapas mudam e como funciona o CI/CD.
1. [arquitetura.md](arquitetura.md): o mapa geral, ou seja, quem fala com quem e o
   caminho de uma busca e de um "play".
2. [diario.md](diario.md): o que foi feito em cada sessão, os problemas que apareceram
   e como foram resolvidos. É a parte mais próxima do dia a dia real de um dev.
3. As **decisões** e os **conceitos** abaixo, conforme a curiosidade.

## Decisões (ADRs)

Um *Architecture Decision Record* é um texto curto que congela uma decisão: o contexto,
o que foi decidido, as alternativas e as consequências. Quando alguém (ou você daqui a
seis meses) perguntar "por que diabos fizeram assim?", a resposta está escrita.

| # | Decisão |
|---|---|
| [0001](decisoes/0001-tauri-em-vez-de-electron.md) | Tauri + Svelte em vez de Electron |
| [0002](decisoes/0002-audio-no-rust.md) | Todo o áudio é tocado no Rust, nunca na WebView |
| [0003](decisoes/0003-login-sem-credenciais-de-desenvolvedor.md) | Login sem credenciais de desenvolvedor (e o caso do Spotify) |
| [0004](decisoes/0004-vidro-uma-vez-so.md) | Glassmorphism com um único desfoque |
| [0005](decisoes/0005-capas-em-dois-tamanhos.md) | Capas em dois tamanhos |
| [0006](decisoes/0006-discord-por-ipc.md) | Discord Rich Presence por IPC local |
| [0007](decisoes/0007-atualizacao-automatica.md) | Atualização automática assinada, pela VPS |

## Conceitos

| Conceito | Onde aparece no código |
|---|---|
| [Comandos e eventos do Tauri (IPC)](conceitos/ipc-tauri.md) | `commands.rs`, `src/lib/api.ts` |
| [Async em Rust: fan-out com timeout](conceitos/async-fan-out.md) | `aggregate.rs` |
| [OAuth 2.0 + PKCE + loopback](conceitos/oauth-pkce.md) | `auth/oauth.rs` |
| [Login por cookie e SAPISIDHASH](conceitos/login-por-cookie.md) | `auth/web_login.rs`, `providers/` |
| [Svelte 5: runes](conceitos/svelte-runes.md) | `src/lib/app.svelte.ts` |
| [Testes que valem a pena](conceitos/testes.md) | `#[cfg(test)]` em vários módulos |
| [Distribuição: web × desktop](conceitos/distribuicao.md) | `.github/workflows/release.yml`, `scripts/` |

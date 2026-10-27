# 0006 — Discord Rich Presence por IPC local

**Status:** aceita (2026-10-26)

## Contexto

O pedido era usar o **Discord Social SDK** para mostrar "Ouvindo GlobalMusic" com a música.

## Alternativas

| Opção | Prós | Contras |
|---|---|---|
| Discord Social SDK | SDK oficial e moderno | Binário em C++ que só sai do Developer Portal com login, exige escrever bindings Rust↔C++ e muda por plataforma |
| **IPC local** (`discord-rich-presence`) | Rust puro, multiplataforma, mesmo card visual | Só funciona com o app desktop do Discord aberto na mesma máquina |

## Decisão

IPC local. O app do Discord abre um *named pipe* (Windows) ou um *socket Unix*
(macOS/Linux) chamado `discord-ipc-0`. Qualquer programa da máquina pode mandar
"defina a atividade como X", e o Discord mostra isso no perfil.

O card usa o mesmo modelo de *Activity* do Social SDK. Se um dia trocarmos, só o
`presence.rs` muda.

## Como funciona no código (`presence.rs`)

- **Thread própria:** o IPC é bloqueante, ou seja, espera o Discord responder. Numa thread
  separada, isso nunca trava a interface nem o áudio.
- **Canal (`mpsc`):** o resto do app só faz `presence.show(&snapshot)`, que manda uma
  mensagem para a thread, e nunca espera.
- **Reconexão:** se o Discord não está aberto, a thread tenta de novo a cada 15 s. Assim,
  abrir o Discord depois do GlobalMusic funciona.
- **`ActivityType::Listening`** faz aparecer "Ouvindo", não "Jogando".
- **`StatusDisplayType::Name`** faz o status mostrar **"Ouvindo GlobalMusic"**, o nome do
  app cadastrado no portal. (A primeira versão usava `Details`, que mostrava "Ouvindo
  <nome da música>". O dono do projeto preferiu a marca do app, como faz a extensão do YouTube Music.)
- **Layout do card**, igual ao dos players de música: título (`details`), "por Artista"
  (`state`), álbum (`large_text`, que repete o título quando não há álbum), capa e barra de tempo.
- **Timestamps de início e fim** fazem o Discord desenhar a barra de progresso sozinho.
- **Botão "Abrir no Spotify/YouTube Music"** e título clicável (`details_url`). Os botões
  aparecem para **os outros**: no seu próprio perfil o Discord não os mostra clicáveis.

## Consistência: os quatro problemas e as soluções

| Problema | Sintoma | Solução |
|---|---|---|
| Posição desconhecida | Pausar e retomar reiniciava a barra no zero | O `Player` ganhou um relógio (`player/clock.rs`) e virou a fonte da verdade da posição |
| Limite do Discord (5 / 20 s) | Pulando faixas rápido, o card ficava preso numa música antiga | **Coalescência** (rajadas em 400 ms viram um envio) + **intervalo mínimo** de 4 s |
| Envio atrasado | A barra ficava atrás da música real | O snapshot calcula um **horário absoluto** de início (`agora - posição`); atrasar o envio não muda esse horário |
| Envios inúteis | Mexer no volume reenviava o card | `same_card` ignora mudanças que não alteram faixa nem início (tolerância de 1,5 s para jitter) |

### O relógio sem "tick" (`player/clock.rs`)

Um jeito ingênuo de saber a posição é somar +1 s a cada segundo. Isso acumula erro e gasta CPU.
Guardamos só `base_ms` (quanto já tocou) e `running_since` (desde quando está tocando). A
posição é **calculada na hora em que alguém pergunta**: `base + (agora - running_since)`.
Pausar "congela" (`base += tempo corrido`), e o seek troca a `base`. Os testes passam
instantes fabricados (`t0 + 3s`), então não precisam esperar o relógio de verdade.

### Seek na interface: prévia × commit

Arrastando a barra, só a UI se move (`previewSeek`). Ao **soltar** (`oncommit`), o backend
recebe o seek, e o Discord é atualizado. Assim, arrastar não dispara dezenas de chamadas.

## Configuração (uma vez, pelo dono do projeto)

1. Em discord.com/developers/applications, crie um app chamado **GlobalMusic**. O nome
   aparece em "Ouvindo GlobalMusic". ✅ ID `1564155693282496582`, já em `config.rs`.
2. Em *Rich Presence → Art Assets*, envie as quatro imagens de `assets/discord/`, mantendo os
   nomes: `globalmusic`, `spotify`, `youtube_music` e `soundcloud`. O Discord pode levar alguns
   minutos para começar a servir as imagens novas.
3. **Não** é preciso ativar intents (essas opções servem para bots).

Pronto: todo mundo que instalar já usa esse ID. Cada pessoa só precisa do Discord desktop
aberto, da opção "Compartilhar atividades detectadas" ligada e de não estar "Invisível".

## Testando agora, sem esperar a fase 2

```sh
cargo test --manifest-path src-tauri/Cargo.toml live_discord_card -- --ignored --nocapture
```

Esse teste fica marcado como `#[ignore]` (não roda no `npm run test:rust`, porque depende do
Discord aberto). Ele publica um card de exemplo por 20 s e depois limpa.

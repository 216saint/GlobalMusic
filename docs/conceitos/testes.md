# Testes que valem a pena

Rodar: `npm run test:rust` (26 testes, menos de 1 s).

## O que testamos, e por que isso e não outra coisa

Testamos as partes em que **um erro seria silencioso**, ou seja, o app não quebra, só faz
a coisa errada:

| Teste | O erro que ele pega |
|---|---|
| `aggregate::search_returns_partial_results...` | Uma fonte fora do ar zerar a busca inteira |
| `resolver::picks_the_original_over_live_and_remix` | Usuário Free ouvir a versão ao vivo no lugar da original |
| `resolver::refuses_when_nothing_is_close_enough` | Tocar uma música aleatória quando não acha a certa |
| `oauth::callback_rejects_wrong_state` | Brecha de segurança (CSRF) no login |
| `presence::clip_respects_discord_limits` | O Discord rejeitar o card por texto > 128 caracteres |
| `artwork::*` | Lista carregando capas de 640px (estouro de RAM) |
| `youtube_music::sapisid_hash_matches_reference` | Todas as requisições logadas do YouTube falhando |

## Padrão: separar lógica pura de efeitos

Quase toda função testada é **pura**: recebe dados e devolve dados, sem rede, disco ou
relógio. Por exemplo:

```rust
fn timestamps(now_ms: i64, position_ms: u64, duration_ms: u64) -> Option<(i64, i64)>
```

O "agora" entra como **parâmetro**, em vez de a função chamar `SystemTime::now()` por
dentro. Assim o teste controla o tempo. O mesmo vale para `sapisid_hash(…, unix_secs)`.

Esse é o padrão "núcleo funcional, casca imperativa": a lógica fica pura e testável, e a
casca fina (rede, janelas, áudio) só junta as peças.

## Fakes em vez de mocks

O teste do `aggregate` cria um `struct Fake(Source, Behavior)` que implementa o trait
`MusicProvider` de verdade, com três comportamentos (ok, falha, trava). Não precisamos de
biblioteca de mocks, porque o trait **é** o ponto de troca. Esse é um dos benefícios de
programar contra uma interface (`MusicProvider`) e não contra o Spotify diretamente.

# Async em Rust: fan-out com timeout

"Buscar nas 3 fontes" pode ser feito de dois jeitos:

```
Sequencial:  Spotify (1,2s) → YouTube (0,8s) → SoundCloud (8s, travado)  = 10s
Paralelo:    Spotify (1,2s)
             YouTube (0,8s)                                             = 1,2s + o que travou
             SoundCloud ─── corta em 8s ───┘
```

*Fan-out* é disparar várias operações ao mesmo tempo e juntar os resultados.

## O código (`aggregate.rs`)

```rust
let calls = providers.iter().cloned().map(|provider| {
    let source = provider.source();
    let call = op(provider);                      // 1. cria a future (ainda não roda!)
    async move {
        match tokio::time::timeout(timeout, call).await {   // 2. corrida contra o relógio
            Ok(Ok(items)) => SourceResults::ok(source, items),
            Ok(Err(e))    => SourceResults::failed(source, e.to_string()),
            Err(_)        => SourceResults::failed(source, "tempo esgotado"),
        }
    }
});
join_all(calls).await                             // 3. roda todas juntas
```

### Conceitos

- **Future é preguiçosa.** Em Rust, chamar uma função `async` *não* executa nada. Ela
  devolve uma *future*, uma receita. Só ao dar `.await` (ou passar para `join_all`) ela roda.
- **`join_all`** recebe várias futures e as executa de forma concorrente, devolvendo
  todos os resultados **na mesma ordem**. Por isso o teste pode afirmar que `results[0]` é o Spotify.
- **`tokio::time::timeout`** embrulha uma future: se ela não terminar a tempo, devolve
  `Err(Elapsed)` e a future é *cancelada* (descartada).
- **Três desfechos, três tipos:** `Ok(Ok(_))` é sucesso, `Ok(Err(_))` é o provedor que
  falhou e `Err(_)` é o estouro de tempo. O compilador obriga a tratar todos no `match`.
- **`Arc<dyn MusicProvider>`**: `dyn` permite guardar provedores de tipos diferentes na
  mesma lista (polimorfismo). `Arc` é um ponteiro com contagem de referências que pode ser
  compartilhado entre tarefas. O `.cloned()` copia o **ponteiro**, não o provedor.

## Por que é uma função genérica

`fan_out` recebe a operação como closure (`op`). A busca e a biblioteca usam a **mesma**
lógica de paralelismo e timeout, só mudando o que se pede a cada provedor. Uma regra
escrita uma vez, testada uma vez.

## Como o teste não demora 8 segundos

```rust
#[tokio::test(start_paused = true)]
```

O Tokio tem um **relógio falso** para testes. Com o tempo pausado, quando todas as
tarefas estão esperando, ele *avança o relógio* instantaneamente. O provedor que "trava
para sempre" (`std::future::pending()`) estoura o timeout de 8 s em microssegundos.

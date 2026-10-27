# Comandos e eventos do Tauri (IPC)

O app tem **dois mundos** que não compartilham memória: o JavaScript, na WebView, e o
Rust, no processo do app. Eles conversam por **IPC** (*Inter-Process Communication*),
trocando mensagens serializadas em JSON.

## Comandos: o frontend pergunta e o Rust responde

```rust
// src-tauri/src/commands.rs
#[tauri::command]
pub async fn search(state: State<'_, AppState>, query: String, limit: Option<u32>)
    -> Result<Vec<SourceResults<Track>>>
```

```ts
// src/lib/api.ts
invoke<SourceResults<Track>[]>("search", { query, limit })
```

O que acontece:

1. O `invoke` serializa `{ query, limit }` em JSON e manda para o Rust.
2. O Tauri desserializa nos parâmetros da função. O nome em camelCase no JS
   (`playlistId`) vira snake_case no Rust (`playlist_id`) automaticamente.
3. `State<'_, AppState>` **não vem do JS**. O Tauri injeta o estado registrado com
   `app.manage(...)` em `lib.rs`. É *injeção de dependência*.
4. O retorno vira JSON. Um `Err` vira uma Promise rejeitada no JS. Por isso `error.rs`
   implementa `Serialize`, para o erro chegar como texto legível.

**Por que `async`?** Comandos síncronos rodam na thread principal, que é a da janela.
Uma busca de 3 s congelaria a interface. Comandos `async` rodam no runtime Tokio.

**Por que todo comando devolve `Result`?** Comandos `async` que recebem referências
(`State<'_, ...>`) precisam devolver `Result`. É uma exigência do Tauri, ligada aos
*lifetimes* do Rust.

## Eventos: o Rust avisa sem ninguém perguntar

```rust
app.emit("player://state", &snapshot)            // Rust → todas as janelas
```
```ts
listen<PlayerSnapshot>("player://state", (e) => …) // JS escuta
```

Usado quando o estado muda "sozinho": a música acabou, ou um atalho de mídia do teclado
pulou de faixa. O frontend não precisa ficar perguntando (*polling*).

## Tipos dos dois lados

`src/lib/types.ts` espelha `models.rs` à mão. Se um lado mudar e o outro não, quebra em
runtime. Um próximo passo de aprendizado é gerar o TS a partir do Rust com o crate `specta`.

## Segurança: capabilities

`src-tauri/capabilities/default.json` diz o que a janela `main` pode fazer (minimizar,
fechar, abrir URLs…). As janelas de login do YouTube e do SoundCloud carregam sites
externos e **não estão** nessa lista. Elas não conseguem chamar nenhum comando nosso.
É o princípio do *menor privilégio*.

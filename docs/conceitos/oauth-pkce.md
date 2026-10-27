# OAuth 2.0 + PKCE + loopback

É o padrão usado quando um app quer agir **em nome** de um usuário num serviço
(ler as playlists dele no Spotify, por exemplo) **sem nunca ver a senha dele**.

## O fluxo (RFC 8252, "OAuth para apps nativos")

```
 GlobalMusic                      Navegador                       Spotify
     │ 1. gera verifier + challenge  │                               │
     │ 2. abre a URL de autorização ─►│ ── usuário faz login ───────► │
     │                                │ ◄── 3. redirect para ──────── │
     │ ◄─ http://127.0.0.1:47823/...?code=XYZ&state=abc              │
     │ 4. troca code + verifier por tokens ─────────────────────────► │
     │ ◄──────────────────────────── access_token + refresh_token ── │
```

1. **Loopback:** o app sobe um mini servidor HTTP em `127.0.0.1:47823`
   (`tauri-plugin-oauth`) só para receber o redirect. Por isso a *Redirect URI* cadastrada
   é um endereço local.
2. **PKCE** (*Proof Key for Code Exchange*), em `Pkce::generate`:
   - `verifier` = 64 bytes aleatórios (segredo que nunca sai do app até o passo 4);
   - `challenge` = SHA-256 do verifier, que vai na URL do passo 2.

   Se alguém interceptar o `code` no passo 3, não consegue trocá-lo por tokens, porque não
   tem o `verifier`. É isso que dispensa o *client secret*, que num app desktop seria
   extraível do binário.
3. **`state`** é um valor aleatório que precisa voltar igual no redirect. Ele impede um
   site malicioso de injetar um `code` dele no seu app (CSRF). Veja o teste
   `callback_rejects_wrong_state` em `auth/oauth.rs`.

## Tokens

- `access_token` vale por cerca de 1 h e vai em cada requisição (`Authorization: Bearer ...`).
- `refresh_token` serve para pegar um novo access token sem pedir login de novo.
- São guardados com `keyring`, no **cofre do sistema** (Gerenciador de Credenciais do
  Windows, Keychain no macOS, Secret Service no Linux), e nunca em arquivo de texto.

## Lendo o código

`auth/oauth.rs` → `authorize_via_browser`:

- O `oneshot::channel` é um canal de uma mensagem só. A closure do servidor de loopback
  (que roda em outra thread) manda a URL, e a função `async` espera por ela.
- O `tokio::time::timeout` impede que o app fique esperando para sempre se a pessoa
  fechar o navegador.
- O servidor é cancelado em **todos** os caminhos (sucesso, erro e timeout), para não
  deixar a porta presa.

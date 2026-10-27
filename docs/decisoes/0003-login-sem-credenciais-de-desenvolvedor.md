# 0003 — Login sem credenciais de desenvolvedor

**Status:** aceita (2026-10-26). Revisada no mesmo dia para incluir o Spotify.

## Contexto

Requisito: quem instalar o app deve conseguir entrar com a **própria conta** e usar,
**sem configurar nada**.

Primeiro, a distinção que confunde todo mundo no começo:

- **Client ID** identifica o **aplicativo** para o serviço ("quem está pedindo acesso é o
  GlobalMusic"). Ele é o mesmo para todos os usuários.
- **A conta** é de cada pessoa. Mesmo com um Client ID único, cada um faz o próprio login.

O problema é que os serviços colocam regras sobre apps registrados por desenvolvedores:

| Serviço | Restrição de app registrado |
|---|---|
| Spotify | "Development mode": só e-mails cadastrados no dashboard conseguem logar (lista pequena). Sair desse modo exige ser empresa |
| Google | Tela de consentimento em "teste": no máximo 100 usuários cadastrados, aviso de "app não verificado" e o login expira em 7 dias |
| SoundCloud | O registro de apps fica fechado por longos períodos |

## Decisão

**Resultado: o app não tem nenhum arquivo de configuração.** Nada de `.env`.

### YouTube Music e SoundCloud: login pela página oficial, sessão por cookie

Abrimos a **página de login oficial** do serviço numa janela do próprio app
(`auth/web_login.rs`). A pessoa entra normalmente, inclusive com 2FA, e, quando o cookie de
sessão aparece, a janela fecha. Depois disso, o backend faz as mesmas requisições que o site
oficial faz, com os cookies da pessoa. Detalhes em
[login-por-cookie](../conceitos/login-por-cookie.md).

### Spotify: client ID do app desktop do Spotify, com auto-atualização

Usamos o **mesmo client ID que o `librespot` usa**: o do cliente desktop do próprio Spotify.
Ele não está sujeito ao "development mode", então qualquer conta (Free ou Premium) consegue
logar. O login é OAuth + PKCE comum ([oauth-pkce](../conceitos/oauth-pkce.md)), com redirect
em `http://127.0.0.1:47823/login`, o único formato que esse cliente aceita.

**Histórico da decisão (vale como aprendizado):** O
dono do projeto pesou os argumentos (contas legítimas, dados da própria pessoa, Premium
continua pago ao Spotify, projeto sem fins lucrativos) e **decidiu seguir conscientemente**.
É o mesmo caminho de projetos open source como `librespot`, `spotifyd` e `ncspot`.

#### Auto-atualização do client ID (`providers/spotify/client_id.rs`)

Se o Spotify trocar esse ID, o projeto `librespot` atualiza o código-fonte dele. Então o app:

1. uma vez por dia (em segundo plano, ao abrir, e antes de cada login), baixa
   `core/src/config.rs` do repositório do librespot no GitHub;
2. extrai `KEYMASTER_CLIENT_ID` e **valida** que tem exatamente 32 caracteres hexadecimais,
   porque o que vem da internet nunca é usado sem checagem;
3. guarda em cache (`spotify_client_id.json` na pasta de dados do app);
4. se nada disso funcionar, usa o ID embutido em `config.rs`.

Os tokens guardam o client ID que os emitiu (`Tokens::client_id`). A renovação usa esse mesmo
ID. Se o Spotify recusar (ID revogado), o app apaga os tokens e mostra "conecte de novo".

#### Limite de requisições (429)

**Quem define como o limite é contado é o servidor do Spotify, não o app.** O Spotify conta
**por client ID**, numa janela de 30 s. Com um client ID compartilhado por todos os
usuários do librespot e afins, a cota é compartilhada, e não há código do nosso lado que
mude isso.

O que fazemos:

- **`providers/spotify/http.rs`:** ao receber 429, espera o `Retry-After` que o Spotify manda
  e tenta de novo (até 3 vezes, esperando no máximo 20 s). Só depois avisa a UI.
- **Fase 2:** cache local da biblioteca (que muda pouco) e debounce na busca, para cada
  usuário gastar o mínimo de chamadas.
- **"Usar meu próprio Client ID"** (implementado depois do primeiro teste real, ver abaixo): na
  tela de perfil, a pessoa pode colar o Client ID de um app que ela mesma criou no dashboard. A
  cota passa a ser **exclusiva** dela, e essa é a única forma real de ter limite individual.
  Como ela é a dona do app, o "modo de desenvolvimento" não a bloqueia.

#### O que o primeiro teste real mostrou

Na primeira tentativa de login, o Spotify respondeu **429 com `Retry-After: 47`** já na
primeira chamada (`/me`). A cota do client ID compartilhado estava esgotada pelos outros
usuários do librespot e afins. Ou seja, o risco documentado acima é real, não teórico.

Ajustes feitos:

1. **Bug corrigido:** o login salvava o token e depois falhava ao ler o perfil, mostrando
   "não conectado". Agora, com o token salvo, a conta conta como conectada, e o perfil é lido
   quando a cota permitir.
2. **Perfil em cache por 15 min**, porque abrir a tela de perfil não deve gastar cota.
3. **Client ID próprio** na tela de perfil, aberto automaticamente quando aparece o 429. Ao
   trocar o ID, os tokens antigos são apagados, porque um token só vale para o client que o
   emitiu.

O padrão continua sendo "funciona sem configurar nada". O Client ID próprio é a válvula de
escape para quando a cota compartilhada não der conta.

### Discord: ID público embutido

O Application ID do Discord é público, e não existe login. Basta criar o app **uma vez** e
colar o ID em `config.rs` (`DISCORD_APP_ID`). Ver ADR 0006.

## Consequências

- ✅ Qualquer pessoa instala, conecta as próprias contas e usa, sem configurar nada.
- ⚠️ APIs internas (InnerTube, api-v2) e o client ID do Spotify podem mudar sem aviso. Cada
  serviço está isolado no seu provedor, e a busca federada continua funcionando com as outras
  fontes quando uma quebra.
- ⚠️ O Google às vezes bloqueia login em janelas embutidas ("este navegador pode não ser
  seguro"). O plano B é abrir o login no navegador do sistema e importar a sessão.
- ⚠️ A cota de requisições do Spotify é compartilhada (ver acima).

# Diário de desenvolvimento

Registro do que foi feito em cada sessão e, principalmente, dos **problemas reais** que
apareceram e de como foram resolvidos. Esse tipo de problema não aparece em tutorial.

---

## Sessão 1 (2026-10-26): infraestrutura

### O que foi feito

1. **Requisitos:** player global leve, com meta de RAM < 150 MB, busca nas 3 fontes,
   biblioteca unificada com selo de origem, tema glass e suporte a Windows, macOS e Ubuntu.
2. **Stack:** Tauri 2 + Svelte 5 (ADR [0001](decisoes/0001-tauri-em-vez-de-electron.md)).
3. **Scaffold:** `create-tauri-app` gerado numa pasta temporária e trazido para o repo.
   Trocamos o SvelteKit gerado por Svelte + Vite puro, porque roteador e SSR não fazem
   sentido num app desktop de uma tela.
4. **Backend:** modelos, trait `MusicProvider`, busca e biblioteca federadas, fila,
   roteamento Premium/Free, resolver do fallback, OAuth PKCE, cofre de tokens, capas e Discord.
5. **UI:** layout de dois painéis inspirado na referência, com tema glass e a linguagem
   visual do ícone (barras escalonadas).
6. **Medição:** build de release com 3,5 MB e **106,5 MB de RAM** com o app aberto.
7. **Login sem credenciais** para YouTube Music e SoundCloud (ADR
   [0003](decisoes/0003-login-sem-credenciais-de-desenvolvedor.md)).

### Problemas e soluções

#### 1. `cargo` "não reconhecido" no terminal
**Sintoma:** `cargo --version` falhava, mas o Rust estava instalado.
**Causa:** o `rustup` instala em `%USERPROFILE%\.cargo\bin` e adiciona esse caminho ao
PATH do usuário, mas terminais que **já estavam abertos** não enxergam a mudança.
**Solução:** reabrir o terminal (ou o VS Code).

**Atualização (primeiro teste real):** o mesmo erro apareceu no terminal do VS Code
(`cargo metadata ... program not found`). O PATH do usuário no Windows **tinha** o
`.cargo\bin`, mas o VS Code foi aberto antes da instalação do Rust. Ele guarda as variáveis de
ambiente de quando abriu, e todo terminal novo herda essa cópia antiga. Solução: fechar e
abrir o VS Code inteiro (abrir um terminal novo não basta). Para o terminal atual:
`$env:Path = [Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [Environment]::GetEnvironmentVariable("Path","User")`.
**Lição:** variável de ambiente é uma *cópia* que cada processo recebe ao nascer. Mudar a
original não atualiza quem já estava rodando.

**Correção definitiva:** em vez de depender de cada pessoa reiniciar o terminal, os scripts
`npm run app:*`, `tauri` e `test:rust` passam por `scripts/with-cargo.mjs`, que acrescenta
`~/.cargo/bin` ao PATH do processo filho quando falta. Resolver o problema para todo mundo é
melhor do que documentar um passo manual que alguém vai esquecer.

#### 2. Build do `librespot` quebrando: `vergen_lib::entries::Add is not satisfied`
**Sintoma:** erro no `build.rs` do `librespot-core`, falando de "multiple different
versions of crate `vergen_lib`".
**Causa:** o `librespot-core` usa `vergen-gitcl` 1.0, que pede `vergen` "^9.0.6". O Cargo
escolheu o `vergen` 9.1.0, que trocou uma dependência interna (`vergen-lib` 0.1 → 9.x).
Pelo *semver*, 9.0 → 9.1 deveria ser compatível, mas não foi. Foi uma quebra acidental
de quem publicou.
**Solução:** fixar a versão no lockfile:
```sh
cargo update -p vergen --precise 9.0.6
```
**Lição:** o `Cargo.lock` existe exatamente para isso, então **commite o lockfile** em
aplicações. E leia a mensagem de erro inteira: o "multiple different versions" era a pista.

#### 3. `npm install` instalou o TypeScript 7
**Causa:** o TS 7 é a reescrita nativa (em Go) do compilador, e ferramentas como o
`svelte-check` ainda dependem da API antiga.
**Solução:** `npm install -D typescript@~6.0.3`. **Lição:** "latest" nem sempre é compatível
com o ecossistema. Fixe versões *major* em dependências de ferramenta.

#### 4. `vite build`: "Cannot find package 'esbuild'"
**Causa:** o Vite 8 trocou o esbuild pelo Rolldown/Oxc. O template do Tauri ainda
configurava `minify: "esbuild"`.
**Solução:** `minify: true`, que usa o minificador padrão da versão.

#### 5. `keyring` 4 com API diferente
O `keyring` 4 reorganizou a biblioteca em vários crates. Para não arriscar uma API que eu
não conhecia bem, usei a 3.x, que tem API estável e documentada. **Lição:** a versão mais
nova nem sempre é a escolha certa para começar.

#### 6. Pasta `target` dentro do OneDrive
O projeto está no OneDrive, e a pasta `target` do Rust passa de vários GB com milhares
de arquivos, o que travaria a sincronização. Ela foi apontada para fora com
`.cargo/config.toml` (`target-dir`). Esse arquivo é local da máquina e fica no `.gitignore`.

#### 7. Chips de filtro quebrando de linha
Visto na captura de tela da janela real: o chip "SoundCloud" ia para a segunda linha.
A coluna esquerda foi fixada em 360px e os chips ficaram com `white-space: nowrap`.
**Lição:** olhe o app rodando de verdade, não só o código.

#### 8. Reaproveitar o client ID oficial do Spotify: barrado
A ideia de usar o client ID do próprio Spotify (o mesmo do `librespot`) para fugir do
"development mode" foi **bloqueada pelo sistema de segurança do assistente**. Detalhes e
alternativas na ADR [0003](decisoes/0003-login-sem-credenciais-de-desenvolvedor.md).
**Lição:** viabilidade técnica e legitimidade são perguntas diferentes, e as duas
precisam de resposta antes de construir.

#### 9. Spotify: decisão consciente pelo client ID do app desktop
Depois do bloqueio do item 8, o dono do projeto pesou os prós e contras e decidiu seguir
(argumentos na ADR [0003](decisoes/0003-login-sem-credenciais-de-desenvolvedor.md)). Foram
implementados o login completo (PKCE, troca de código, renovação de token, `/me` com
Free/Premium) e duas proteções:
- **Auto-atualização do client ID** a partir do código-fonte do librespot no GitHub. Antes de
  confiar no parser, conferimos o arquivo real com `curl ... | grep KEYMASTER_CLIENT_ID`.
  Testar com um texto de exemplo não basta: é preciso saber se o mundo real tem o formato esperado.
- **Tratamento de 429** com `Retry-After`.

**Lição sobre rate limit:** "quero que o limite seja por usuário" é uma decisão do
**servidor**, não do cliente. O Spotify conta por client ID. Quando o requisito depende de
outro sistema, o trabalho é entender as regras dele e projetar em volta delas (cache,
retry e cota própria opcional).

#### 10. O `.env` deixou de existir
Com o Spotify sem credencial própria, o `AppConfig` e o `dotenvy` foram removidos. Menos
configuração significa menos coisa para dar errado na máquina de quem instala.

#### 11. Rich Presence "consistente"
Revendo o `presence.rs` com a pergunta "o que faria esse card mentir?", apareceram quatro
problemas: posição sempre zero, limite de 5 envios / 20 s do Discord, atraso desalinhando a
barra e reenvios inúteis. As soluções estão na ADR [0006](decisoes/0006-discord-por-ipc.md).
A principal mudança de arquitetura: **o player virou a fonte da verdade da posição**
(`player/clock.rs`). Antes, só a UI sabia em que ponto a música estava, e cada parte
"achava" uma coisa.
**Lição:** quando dois lugares guardam o mesmo dado, eles vão discordar. Escolha um dono.

Os assets do Discord (1024×1024) foram gerados a partir dos mesmos SVGs da interface,
com `@resvg/resvg-js`, para o card e o app terem a mesma identidade visual.

Para testar sem a fase 2, criamos um **teste manual marcado com `#[ignore]`**
(`live_discord_card`). É um padrão útil para coisas que dependem do mundo externo: o teste
existe e está documentado, mas não roda na suíte normal.

#### 12. "Ouvindo GlobalMusic", não "Ouvindo <música>"
Eu tinha escolhido `StatusDisplayType::Details` (o status mostra o nome da música), achando que
era o mais informativo. O dono do projeto mandou um print da extensão do YouTube Music como
referência: ele queria a **marca do app** no status. Bastou trocar para `Name` e ajustar as
linhas do card ("por Artista", álbum).
Para não depender de olhar o Discord a cada ajuste, o teste `card_reads_listening_to_globalmusic`
serializa o `Activity` no JSON que vai para o Discord e confere campo a campo.
**Lição:** quando o requisito é visual, peça (ou ofereça) uma referência. Um print vale mais
que três parágrafos de descrição.

#### 13. Fases 2 e 3: dados reais e áudio
**Validar antes de escrever:** antes do parser do YouTube Music, fizemos uma busca real com
`curl` na InnerTube e guardamos uma amostra enxugada da resposta em
`providers/fixtures/ytm_search.json`. O teste `parses_real_search_response` roda contra esse
JSON de verdade, não contra um exemplo inventado por mim.

**Parser à prova de mudança de layout:** em vez de seguir um caminho fixo
(`contents.tabbed...[0]...`), o `innertube.rs` procura **recursivamente** os
`musicResponsiveListItemRenderer`. Se o YouTube mover a lista de lugar, o parser ainda acha.

**Duas coisas que o assistente não pôde fazer** (bloqueadas pelo classificador de segurança):
baixar e executar o yt-dlp, e raspar o `client_id` do SoundCloud para testar. O código existe e
compila, mas a validação ao vivo dessas duas partes ficou para o primeiro teste real.

**Problemas resolvidos no caminho:**
- *Duas versões do `reqwest`*: o `stream-download` usa a 0.13 e nós a 0.12, então os tipos
  `reqwest::Client` das duas são **diferentes** para o compilador ("trait bound not satisfied").
  Solução: subir para a 0.13 (onde `form` e `query` viraram features opcionais).
- *Dependência morta*: o `rspotify` saiu, porque a Web API passou a ser chamada direto.
- *Deadlock em potencial*: `load_spotify` travava o mutex `spotify` e depois o `active`, e o
  `stop` travava na ordem inversa. Duas tarefas assim podem esperar uma pela outra para sempre.
  Regra prática: **sempre travar na mesma ordem**, ou soltar um lock antes de pegar o outro.
- *Recursão de tipos async*: `start_current` → `on_audio_event` → `start_current`. Em Rust, o
  tipo de uma função `async` contém o tipo de tudo que ela aguarda, então as duas conteriam uma
  à outra infinitamente. Solução: a falha volta pelo canal de eventos (`Arc<dyn Fn>`), e o `dyn`
  quebra o ciclo.
- *Andaime removido*: saiu o `#![allow(dead_code)]`, e o compilador apontou três sobras
  (`NotImplemented`, `Source::ALL`, `Queue::enqueue`), que foram apagadas.

**Gerações** (`audio/mod.rs`): resolver uma faixa leva segundos. Se a pessoa pular de novo
nesse meio tempo, o carregamento antigo termina, vê que sua geração não é a atual e é
descartado. Sem isso, duas músicas tocariam ao mesmo tempo.

#### 14. Primeiro teste real: Spotify respondeu 429 no login
Instalado o app, o primeiro "Conectar" no Spotify mostrou *"muitas requisições agora; tente de
novo em 47s"*. Duas causas:
- **Externa:** a cota do client ID compartilhado (o mesmo do librespot) estava esgotada. É o
  risco que a ADR 0003 previa, agora confirmado na prática.
- **Nossa:** o login dava certo (o token era salvo), mas a leitura do perfil logo depois falhava
  com 429, e o `?` propagava o erro, então a tela dizia "não conectado". Um erro *secundário*
  derrubava uma operação que já tinha dado certo.

Correções: o login não falha mais por causa do perfil, o perfil fica em cache, e entrou a opção
"usar meu próprio Client ID" (cota exclusiva).
**Lição:** num fluxo de vários passos, pergunte "a partir de que ponto isto já deu certo?". O
que vier depois desse ponto deve degradar com elegância, não desfazer o sucesso.

Também ficou claro, ao ver o print, por que **ninguém achava o instalador**: ele é gerado fora
do OneDrive. Agora os builds o copiam para `instaladores/` (`scripts/copy-installers.mjs`).

#### 15. Atualização automática
Pedido: parar de reinstalar a cada correção. Escolha do dono do projeto: hospedar na própria
VPS (ADR [0007](decisoes/0007-atualizacao-automatica.md)).
- Par de chaves gerado com `npx tauri signer generate`; a privada fica em `~/.tauri/`, fora do repo.
- **Uma fonte de verdade para cada coisa:** a versão mora no `package.json` (o
  `tauri.conf.json` aponta para ele) e a URL de atualização mora no `tauri.conf.json` (o
  `release.mjs` deriva dela o endereço dos instaladores). Duplicar esses valores seria pedir
  para eles divergirem.
- `with-cargo.mjs` passou a **exportar** `cargoEnv()` e a só executar comandos quando é rodado
  direto (`import.meta.url === pathToFileURL(process.argv[1]).href`, o equivalente em JS ao
  `if __name__ == "__main__"` do Python). Assim o `release.mjs` reaproveita a lógica.
- O `release.mjs` valida tudo **antes** de mexer em arquivos (URL ainda de exemplo, config de
  upload ausente) e restaura a versão se o build falhar. Testamos as duas recusas sem compilar nada.

Primeiro uso real do `npm run release`: a saída mostrou
`node scripts/release.mjs patch --notes Correção no login do SoundCloud`, **sem as aspas**. No
Windows, o npm repassa os argumentos de um jeito que as aspas se perdem, e a nota chegaria só
como "Correção". Solução: juntar todas as palavras depois de `--notes` até a próxima opção.
De quebra, o `--dry-run` passou a restaurar a versão no fim, porque um ensaio não deve deixar rastro.
**Lição:** leia com atenção o que a ferramenta *ecoa*. A linha que o npm imprimiu antes de
rodar já mostrava o bug.

#### 16. VPS configurada pelo assistente (via SSH)
- A VPS usa **Caddy**, não nginx, então o guia genérico da ADR 0007 não servia como estava. Lição:
  **olhe antes de mexer**. A primeira etapa foi só leitura (serviço ativo, portas 80/443, Caddyfile).
- O próprio Caddyfile documentava que o `216saint.com.br` tem **DNS na Hostinger** (nameservers
  `dns-parking.com`), então a chave do Cloudflare não serve para esse domínio. O registro DNS ficou
  para o dono do projeto criar no painel da Hostinger.
- Mudança feita do jeito seguro, seguindo o padrão que já existia no servidor: backup
  `Caddyfile.antes-globalmusic-<data>`, Caddyfile novo montado numa **cópia** em `/tmp`,
  `caddy validate` na cópia, `diff` mostrando só acréscimos, depois substituição, reload e conferência
  de que os outros sites continuavam respondendo com 200.

#### 17. Janela de login: `WebView2 error 0x8007139F`
"Conectar" no YouTube Music/SoundCloud falhava com *"O grupo ou recurso não está no estado correto"*.
Causa: todas as janelas do app compartilham o mesmo perfil do WebView2, e ele exige **as mesmas
opções** em todas. A janela principal tinha as flags de economia de RAM (`additional_browser_args`) e
a de login não tinha. Correção: a mesma constante (`window::WEBVIEW2_ARGS`) nas duas.
**Lição:** "otimização global" (as flags) cria uma regra invisível que todo código novo precisa
seguir. Deixe a regra escrita bem onde ela é definida.

#### 18. Spotify: o que os outros projetos fazem
Pesquisa no Spotifast e no canora (players baseados no librespot): os dois pedem que **cada
usuário crie o próprio app** no dashboard do Spotify, porque a cota do client ID compartilhado do
librespot vive esgotada. O Spicetify não se aplica: ele roda dentro do app oficial do Spotify. Ou
seja, a nossa seção "Client ID próprio" é a mesma solução da comunidade.

#### 19. Subdomínio no ar: DNS pela API do Cloudflare
O domínio foi movido para o Cloudflare. O assistente teve **comandos de busca por credenciais**
bloqueados pelo classificador de segurança (procurar chaves em arquivos parece exploração, mesmo
num servidor do próprio usuário). Quando o dono do projeto **apontou o arquivo exato**
(`~/.config/cloudflare/credentials.env`, `CF_API_TOKEN`), o uso passou a ser direcionado e funcionou.
- Registro `A globalmusic → 84.46.242.26` criado como **DNS only** (nuvem cinza): o Caddy emite o
  próprio certificado, e o `latest.json` não fica em cache no Cloudflare.
- O comando lê o token direto do arquivo na VPS, então a chave nunca aparece nos comandos nem nos logs.
- O Caddy tinha tentado o certificado antes de o DNS existir e ficou num intervalo de espera
  (*backoff*). Um `reload` (seguro, com a configuração já validada) fez ele tentar de novo:
  "certificate obtained successfully".
**Lição:** "busque a chave" e "use a chave que está em X" parecem iguais, mas não são. O segundo é
explícito e auditável. Em segurança, nomear exatamente o recurso é o que separa uso de exploração.

#### 20. Primeiro uso de verdade: 5 bugs, 3 causas (versão 0.1.2)
O dono do projeto usou o app e mandou prints. Primeiro passo: **agrupar por causa, não por sintoma**.

| Sintoma relatado | Causa real | Correção |
|---|---|---|
| Playlist do Spotify: 403 | O Spotify trocou `/playlists/{id}/tracks` por `/items` em 2026 | Tenta `/items`, cai para `/tracks`, e explica quando a playlist é restrita (Daily Mix etc.) |
| "Abri a playlist e não expandiu" | Não havia tela de detalhe: clicar tocava direto | `PlaylistView.svelte`: lista as faixas; tocar é uma escolha separada |
| "Tocou sem som" + "músicas passando sozinhas" + erro do yt-dlp | **Um bug só.** O yt-dlp falhava, o player pulava, e a próxima falhava também (efeito cascata) | Ver abaixo |

O erro completo do yt-dlp (reproduzido localmente com o mesmo vídeo): *"Sign in to confirm you're not a
bot"*. Testamos seis "clientes" do YouTube sem login: todos pediram login ou um *PO Token*, e o IP
levou 429 depois de alguns testes. Conclusão: **sem sessão logada, o YouTube não entrega mais o áudio**.
- O app já tem a sessão do YouTube Music (cookies do login). Agora exporta esses cookies no formato
  `cookies.txt` e passa ao yt-dlp com `--cookies`, num arquivo temporário apagado logo depois.
- **No máximo 3 falhas seguidas.** Depois disso o player para e mostra o motivo, em vez de varrer a
  fila inteira.
- **Estado "carregando".** O relógio só anda quando o som começa (`Player::started`). Antes, a UI
  dizia "tocando" enquanto o yt-dlp ainda trabalhava, e isso é o "tocou sem som".
- **Spotify recusou a faixa (librespot `Unavailable`)?** Agora a mesma música é tentada pelo YouTube
  antes de pular.
- **Log em arquivo** (`%LOCALAPPDATA%\com.globalmusic.app\logs\globalmusic.log`): no app instalado não
  existe terminal, então sem log não dá para saber por que algo falhou.

**Lições:**
1. Cinco sintomas não são cinco bugs. Pergunte "o que aconteceu primeiro?".
2. Um "pula e tenta a próxima" sem limite transforma uma falha numa avalanche. Todo *retry* precisa de teto.
3. Mostrar "tocando" antes de ter som é mentir para a pessoa. Estados intermediários ("carregando")
   fazem parte do produto.

#### 21. Versão 0.1.3: velocidade, controle e fila
**O log resolveu o mistério do "Carregando…".** As faixas `[55]` e `[56]` registravam
"carregando" e nunca "tocando" nem "falhou": o carregamento ficava **pendurado**. O yt-dlp tem
timeout, então a trava estava depois dele, no "tocar enquanto baixa". O servidor do YouTube limita
a velocidade de uma conexão única, e o decodificador de MP4 precisa pular pelo arquivo. Juntos,
os dois esperavam para sempre.
- **Correção:** `audio/fetch.rs` baixa o arquivo inteiro (3–5 MB) em **pedaços de 1 MB, 4 em
  paralelo** (cabeçalho `Range`), com timeout de 45 s, e toca da memória. Seek passa a ser instantâneo.
- **Pré-carregamento:** quando uma faixa começa, a próxima já é baixada (`Audio::preload`), e o
  `librespot` também tem `preload` para o Spotify.
- **Caches:** URLs do yt-dlp por 1 h (cada chamada custa 2–4 s) e o "equivalente no YouTube" de cada
  faixa do Spotify.
- O `stream-download` saiu do projeto (dependência a menos).

**Controle:** o menu do botão direito numa faixa tem Tocar a seguir, Adicionar à fila,
Curtir/Descurtir e Abrir no serviço. "Curtir" grava nas **Curtidas do GlobalMusic**
(`favorites.json`, faixas de todas as fontes) **e** na conta da pessoa no serviço:
- Spotify: `PUT/DELETE /me/tracks`. Exige o escopo `user-library-modify`, então quem conectou antes
  precisa reconectar.
- YouTube Music: `like/like` e `like/removelike` na InnerTube (o mesmo "joinha" do site).
- SoundCloud: `PUT/DELETE /users/{id}/track_likes/{faixa}`.
Se o serviço recusar, a curtida local fica e um aviso explica o motivo.

**Fila multiplataforma:** painel "Fila" (ícone na barra de título), com tocar a partir de uma posição
e tirar da fila. O `Queue` ganhou `push`, `insert_next`, `remove`, `jump` e `peek_next`, com testes.

**Interface:** barra de título com logo + nome + Buscar, Biblioteca e filtros **só como ícones**. Trocar
de fonte com uma playlist de outra fonte aberta volta para a biblioteca daquela fonte.

#### 22. Servidor social no ar (S1 + S2): conta, foto e amigos
Até aqui, tudo rodava no computador de cada pessoa. Ser amigo de alguém exige **um lugar em comum**,
então nasceu `server/`: um servidor Rust (axum + SQLite) na VPS, em
`https://api.globalmusic.216saint.com.br` (ADR 0008).
- **Login sem senha nossa:** pelo **Spotify**, o app manda o token que já tem e o servidor pergunta ao
  próprio Spotify (`GET /v1/me`) de quem ele é. Pelo **Discord**, é OAuth2: o navegador passa pelo
  servidor e volta ao loopback do app (`127.0.0.1:47823/login`), o mesmo caminho do login do Spotify.
- **Sessão:** um token aleatório de 256 bits, guardado no cofre do sistema. O banco guarda só o
  **SHA-256** dele, então um vazamento do banco não entrega sessões.
- **Foto:** o app recorta e reduz a foto num `<canvas>` (512 px, JPEG) e a manda como **bytes crus**
  pelo IPC (`InvokeBody::Raw`), sem montar um JSON com milhões de números. O servidor decodifica de novo
  (o que recusa o que não for imagem), reduz para 256 px e regrava sem os metadados (GPS etc.).
- **Amizades:** pedido por @usuário. Pedir de volta a quem te pediu **aceita** o pedido. A lista se
  atualiza a cada 60 s, e um ponto no botão de perfil avisa quando há pedido novo.
- **Na VPS:** usuário de sistema próprio (`globalmusic`), serviço systemd com `ProtectSystem=strict`
  e limite de 256 MB (usa ~5 MB), segredos em `/etc/globalmusic/api.env` (só root e o serviço leem),
  Caddy com HTTPS automático e as fotos servidas direto do disco, com backup do Caddyfile antes de
  mexer. O DNS foi criado pela API do Cloudflare.
- **Erro que pegamos no teste:** um token falso do Spotify respondia 401 "faça login". O app usa o 401 para
  saber que a **sessão do GlobalMusic** expirou, então isso deslogaria a pessoa à toa. Passou a ser 400
  com "reconecte o Spotify".
- **Perfil em duas colunas:** na primeira versão, tudo ficava numa coluna de 560 px e sobrava metade da tela
  vazia. Agora é um `grid` com `repeat(auto-fit, minmax(min(100%, 380px), 1fr))`: conta e amigos de um
  lado, fontes de música do outro. Em janela estreita, a segunda coluna desce sozinha, sem media query.

#### 23. "Carregando…" eterno: medir antes de corrigir
**Sintoma:** músicas do YouTube demorando muito para começar. **Medição** (antes de mexer em código):

| Etapa | Tempo |
|---|---|
| yt-dlp só para *iniciar* (o `.exe` "onefile" se descompacta a cada vez) | 1,4 s |
| yt-dlp achando a URL do áudio | 4,2–5,5 s |
| download de 3,4 MB em 4 pedaços paralelos | ~1–3 s |

O log mostrou o resto: **20 carregamentos em 12 segundos** (pulando faixas) e a mesma faixa
carregada **duas vezes ao mesmo tempo** (duplo clique = dois "tocar"). As causas:
1. **Carregamento velho não era cancelado.** Ele só via que não era mais a geração atual *depois*
   do yt-dlp e do download inteiros. Pular 5 faixas deixava 5 yt-dlp e 5 downloads disputando
   CPU e rede com a faixa certa. **Correção:** a geração virou um `tokio::sync::watch`; o carregamento
   faz `select!` entre "baixar" e "outra faixa foi pedida". Quando a segunda acontece, o futuro do
   download é descartado, e descartar um futuro em Rust **cancela** o trabalho dele (o
   `kill_on_drop` encerra o yt-dlp).
2. **Duplo clique** carregava a mesma faixa duas vezes. Agora, se ela já está carregando, o segundo
   "tocar" reaproveita o carregamento em andamento.
3. **Pré-carga e play da mesma faixa não se falavam:** apertar "próxima" no meio da pré-carga abria
   outro yt-dlp. **Correção:** *single-flight*, em que pedidos simultâneos do mesmo vídeo esperam
   **uma** chamada só (`futures::Shared`). O mapa guarda uma referência **fraca** (`WeakShared`):
   se ninguém mais espera, a chamada morre em vez de rodar à toa.
4. **Só a próxima faixa era preparada.** Agora a seguinte é baixada inteira e as duas depois dela
   têm a URL resolvida (`warm`), uma de cada vez e abandonadas se a pessoa trocar de faixa.

**Cuidado de segurança que apareceu no caminho:** os cookies do YouTube vão para o yt-dlp num arquivo
temporário, que só era apagado *depois* que o yt-dlp terminava. Com o cancelamento, ele ficaria
esquecido na pasta temporária. Agora é um tipo com `Drop` (apagado mesmo se cancelado), e o app
varre arquivos que tenham sobrado ao abrir.

O log passou a mostrar o tempo de cada carregamento ("tocando … (5,2 s)") e "cancelado" no lugar
do "tocando" enganoso das faixas descartadas.

#### 24. Música mais rápida: preparar antes do clique, yt-dlp onedir, cache em disco e tocar antes de baixar tudo
Quatro mudanças, cada uma atacando uma parte da espera medida no item 23:
1. **yt-dlp "onedir"** (executável + pasta `_internal/`) no lugar do arquivo único. O arquivo único
   se descompacta a cada execução. Medido no mesmo PC: **iniciar caiu de 1,15–2,36 s para 0,42–0,50 s**
   e resolver uma URL, de 4,3–4,9 s para 3,9–4,0 s. Como agora é uma pasta, o instalador a leva em
   `bundle.resources` (e não em `externalBin`, que só aceita um arquivo), e o app a procura em
   `<recursos>/yt-dlp/`.
2. **Preparar antes do clique:** ao aparecer uma busca, playlist ou curtidas, as 4 primeiras faixas já
   têm a URL resolvida. Parar o mouse 350 ms numa faixa também a prepara. São no máximo 2 yt-dlp em
   segundo plano (`Semaphore`). Cada motivo tem a sua "vez": uma lista nova abandona o que faltava da
   anterior. Se a pessoa clicar numa faixa sendo preparada, o play espera a **mesma** chamada
   (single-flight do item 23).
3. **Cache de URLs em disco** (`youtube-urls.json` na pasta de cache do app). Vale até 30 min antes do
   `expire=` que o próprio YouTube põe na URL (~6 h), em vez de 1 h fixa só na memória. Se o IP mudar e
   a URL der 403, o app esquece ela e pede outra.
4. **Tocar com o primeiro pedaço:** `fetch::stream` devolve um `Incoming` (buffer que vai enchendo com
   4 conexões em paralelo) assim que o primeiro 1 MiB chega (~1 min de música). O detalhe que fazia
   isso travar antes: o decodificador de MP4, quando acha que pode pular pelo arquivo, lê o cabeçalho
   de **todos** os blocos ao abrir (li o código do symphonia: `try_new` percorre o arquivo inteiro).
   Então abrimos **sem seek**, e ele lê só até o primeiro bloco de áudio. Arrastar a barra espera o
   download acabar e **reabre** o arquivo inteiro com seek na mesma saída, mantendo volume e pausa
   (`Cmd::Reopen`). O download para sozinho quando ninguém mais segura o buffer (referência fraca).

E o Discord: o erro "ainda não configurado" não era o redirect, e sim o **Client Secret** vazio no
servidor. Aproveitei para corrigir a experiência: qualquer erro no fluxo do Discord agora **volta ao
app** com o motivo (antes aparecia um JSON no navegador e o app esperava 5 minutos).

#### 25. Spotify tocando pelo Spotify de novo, fila de verdade e aba Amigos
**Spotify lento: a causa não era velocidade.** O log mostrou que **20 de 22** faixas do Spotify eram
recusadas pelo librespot com `StatusCode(400)` e caíam no YouTube (busca do equivalente + yt-dlp,
5–9 s). Lendo o código do librespot, o 400 vem de `POST /extended-metadata/v0/extended-metadata`,
onde ele busca os dados da faixa antes de tocar. Esse endpoint só aceita tokens do client ID do app
oficial (keymaster), e às 08:33 tinha sido salvo um **client ID próprio** (do dashboard, para fugir
do 429). Desde então o librespot conectava com ele.
- **Correção: dois tokens com papéis separados.** O da Web API (biblioteca, busca, curtidas) usa o client
  ID próprio, com a cota só da pessoa. O de **reprodução** usa o keymaster, que é o único que o Spotify
  aceita para tocar e que não gasta a cota da Web API. Com client ID próprio, conectar faz dois logins
  seguidos; o segundo costuma ser só um "Concordo", porque o navegador já está logado. Cada token é
  renovado com o client que o emitiu (`TokenStore::save_as/load_as`).
- **Sessão do librespot aberta antes do primeiro play** (`Audio::warm_spotify`): ao saber que a conta é
  Premium, o app já conecta, e a primeira faixa não paga esses 1–2 s.
- O Perfil avisa quem precisa reconectar (`AccountStatus.notice`).

**Fila no modelo do Spotify** (`player/queue.rs`): agora são duas listas. O **contexto** é a playlist ou
busca de onde se deu play. A **sua fila** é o que foi posto à mão, toca antes do resto do contexto, e
depois o contexto continua de onde parou. Antes era uma lista só, e "Adicionar à fila" jogava a música
no fim da playlist. A fila é endereçada por `Slot` (`{"list":"queued"|"context","index":n}`), e o painel
mostra "Na sua fila" e "A seguir de <playlist>".

**Bug escondido no menu do botão direito:** no Svelte 5, um `{@const track = menu.track}` é
**recalculado** a cada leitura. A ação lia `track` depois de o menu fechar (`menu = null`) e quebrava
em silêncio (achei pelo console do navegador, na demo). Agora a faixa é lida antes de fechar e passada
para a ação.

**Interface:** "Amigos" virou uma aba própria (ícone na barra, com o ponto de pedido pendente). A barra
de título virou um grid `1fr auto 1fr`, que deixa navegação e filtros no **centro da janela** de verdade.

**Discord:** o Client Secret foi para o `/etc/globalmusic/api.env` (sem aparecer em nenhuma saída).
Agora o `/auth/discord/start` redireciona para o Discord.

#### 26. Cache de áudio em disco, letras, SponsorBlock, "escolher versão" e o atalho que tiramos
**SoundCloud 404:** a faixa já não tinha MP3, só AAC em HLS (`aac_160k`) e uma playlist adaptativa
(`audio/mpegurl`). O app procurava MP3 com `starts_with("audio/mpeg")`, e **`audio/mpegurl` também
começa assim**: ele pedia a playlist adaptativa achando que era MP3 e levava 404. Agora compara o tipo
exato e toca AAC: a playlist traz um `init.mp4` (`#EXT-X-MAP`) e pedaços `.m4s`, que juntos formam um
MP4 fragmentado comum.

**Spotify:** o 400 sumiu (o token de reprodução funcionou), mas o Spotify passou a recusar a **chave de
áudio** (`error audio key 0 1`) a clientes não oficiais. Não dá para resolver do nosso lado. Depois de 2
recusas, o app para de tentar o librespot na sessão e as faixas vão direto pelo YouTube, onde agora
ganham preparação e pré-carga. Antes, por serem "Premium", elas não ganhavam nada disso.

**O atalho que tiramos:** medimos que a API interna do YouTube (cliente ANDROID_VR) entrega o começo da
música em ~0,35 s, mas só o 1º MiB: o resto exige um "PO token". O teste de ponta a ponta deu 403 no
resto, e investigar mais seria contornar a proteção anti-bot do YouTube. A verificação automática de
permissões bloqueou exatamente isso, e decidimos não seguir: o código saiu. Fica a lição de método: o
teste de rede de ponta a ponta achou o problema antes de qualquer pessoa usar.

**Cache de áudio em disco** (`audio/cache.rs`): toda música baixada por inteiro fica guardada, e na próxima
vez toca na hora. Leve: teto de 300 MB, e passou disso saem as tocadas há mais tempo (cada play atualiza a
data do arquivo). Só guarda download completo (`Incoming::when_complete`). A pré-carga da próxima faixa,
que antes ficava na RAM (3–5 MB), agora vai para o disco.

**Equivalência salva + "Escolher versão do YouTube"** (`audio/matches.rs`): a faixa do Spotify → vídeo do
YouTube fica num arquivo; abrir o app de novo não refaz as buscas. Pelo botão direito, a pessoa troca uma
versão errada (ao vivo, cover…). A escolha dela nunca é sobrescrita pela busca, e a versão antiga sai do
cache em disco.

**Letras (LRCLIB)** (`lyrics.rs`): a busca exata do LRCLIB devolveu, para uma música famosa, um registro de
teste (`[00:00.00]probe`). Por isso usamos a busca aberta, descartamos registros com menos de 4 versos e
ficamos com o de duração mais próxima. Clicar num verso pula até ele. Um bug de layout no caminho:
`scrollIntoView` rola **todas** as caixas da tela (e escondia os controles), e `padding: 30%` é calculado
pela **largura**, o que fazia a caixa da letra crescer.

**SponsorBlock** (`sponsorblock.rs`): pula trechos sem música dos clipes do YouTube. A consulta usa só os
4 primeiros caracteres do SHA-256 do vídeo (vêm vários vídeos e escolhemos o nosso localmente), então o
servidor não sabe o que a pessoa ouve.

**Spotube:** estudamos o projeto (AGPL-3.0). Aproveitamos **ideias** (cache em disco, equivalência salva,
LRCLIB, SponsorBlock, escolher versão), todas escritas do zero. Copiar código obrigaria o GlobalMusic
inteiro a virar AGPL.

#### 27. "Começou em 2–3 s", fila lateral, importar por link e sincronizar o perfil
**Começava em 2–3 s e demorava:** quando o Spotify recusava a chave de áudio, o relógio do player já
andava (o librespot tinha "aceitado" a faixa). A recuperação pelo YouTube usava essa posição como ponto
de partida. Isso tinha dois efeitos: a música começava no meio, e começar do meio exige seek, então
o app esperava o download inteiro em vez de tocar com o primeiro pedaço. Agora o player volta para
"carregando" no zero (`Player::reload`). A desistência do librespot fica salva por 24 h num arquivo,
e abrir o app de novo não gasta mais duas tentativas de ~3 s.

**Client ID próprio do Spotify não serve para conta Free:** desde fevereiro de 2026, o Spotify só deixa
conta **Premium** criar Client ID (modo de desenvolvimento, até 5 usuários). Torná-lo obrigatório
deixaria quem é Free sem Spotify. Por isso ele continua opcional, com um passo a passo mais claro: aviso
de Premium, botão que abre o painel, botão de copiar o endereço de retorno e conferência do formato.

**Importar playlist por link** (`imports.rs`): o nome e a capa vêm do **oEmbed** (o endereço público que
os sites usam para prévias de link), sem login e sem gastar cota. No SoundCloud, o link não tem o id, e
o `/resolve` da API faz a conversão. As faixas só são buscadas ao abrir.

**Playlist de outra pessoa com Client ID próprio:** um Client ID criado no dashboard fica em Development
Mode, e desde fevereiro de 2026 o Spotify só entrega as faixas de playlists que a pessoa criou ou colabora.
As dos outros chegam sem `items`, então a playlist importada abriria vazia. Por isso as importadas são lidas
com o token de reprodução, que é do client oficial e não tem essa restrição
(`SpotifyProvider::shared_playlist_tracks`). Sem Client ID próprio nada muda, porque o token de sempre já é
o do client oficial. Não custa requisição extra: o app sabe de antemão que a playlist é importada.

**Fila carregando de verdade (lido no log da 0.5.0):**
- **Pular matava a preparação.** A pré-carga da fila era cancelada a cada troca de faixa. O yt-dlp
  é compartilhado (`WeakShared`) e morre quando ninguém mais o segura, então pular para a música que
  estava sendo preparada matava o yt-dlp dela no meio, e o play começava do zero: "I Wanna Be Honest"
  levou 4,9 s. Agora a pré-carga não é cancelada e o play reaproveita o mesmo yt-dlp.
- **A mesma faixa baixava duas vezes** ("pré-carregada" aparecia duas vezes no log). Um conjunto de
  pré-cargas em andamento (`Preloading`, que se desmarca no `Drop`) evita a repetição.
- **Mais à frente:** as 2 próximas são baixadas inteiras e a 3ª tem a URL resolvida, tudo em paralelo
  (antes era só 1, uma depois da outra). "Adicionar à fila" também passa a pré-carregar.
- **librespot:** desiste na **primeira** recusa da chave de áudio (antes eram 2, cada uma custando ~3 s)
  e só tenta de novo depois de 24 h.

**"Carregando…" eterno com música do cache:** a resposta do comando "tocar" leva um snapshot tirado
*antes* de a música começar ("carregando"). Do cache, a música começa em 0 s, e o evento "começou"
chegava à tela antes da resposta. A resposta atrasada sobrescrevia o estado certo. Agora cada snapshot tem
um número de versão (`Player::touch`), e a UI descarta um snapshot mais velho que o que já mostra.

**Teclas de mídia** (`media_keys.rs`, biblioteca `souvlaki`): o app se registra no painel de mídia do
sistema (SMTC no Windows, "Agora tocando" no macOS, MPRIS no Linux), que é por onde o Windows entrega
Fn+F8 e as outras teclas de mídia ao player que está tocando. Atalho global seria pior: briga com outros
apps e não aparece no painel. Os controles vivem na thread principal (`run_on_main_thread`), porque o
macOS exige isso.

**Usar sem conta:**
- Playlists do GlobalMusic (`local_playlists.rs`): faixas de qualquer fonte, criadas pelo card "+" da
  biblioteca ou pelo menu da faixa ("Adicionar à playlist"). As sugestões (tocadas por último e a última
  busca) ficam no `localStorage`, porque são só uma conveniência deste computador.
- Playlist do Spotify importada sem conta: as faixas vêm da página pública do player incorporável
  (`open.spotify.com/embed/...`, a mesma que blogs usam) e tocam pelo YouTube.
- Curtir sem a conta daquele serviço não mostra mais erro: a curtida fica no GlobalMusic.
- Playlists feitas pelo próprio Spotify ("This Is", Daily Mix, Descobertas da Semana) são bloqueadas para
  apps de terceiros na Web API desde nov/2024 e davam erro. Agora, quando a API responde 403, as faixas vêm
  da mesma página pública. Depois, se der, são completadas com capa e álbum por `GET /tracks?ids=` (50 por
  chamada). Sem isso, a playlist abre do mesmo jeito, só sem as capas.
- E essas playlists automáticas **não aparecem mais na Biblioteca** (dono com id `spotify`): a pessoa não
  as montou. A leitura pela página pública continua servindo para quem importar uma delas por link.
- A busca já funcionava sem conta no YouTube Music e no SoundCloud.

**Interface:**
- Abre na Biblioteca.
- Botão para esconder o painel da esquerda (fica só o player); as abas da barra de título o reabrem.
- Maximizada ou em tela cheia, a janela perde os cantos arredondados, que deixavam ver o fundo nas quinas.
- "Buscar atualizações" no rodapé do Perfil (o estado é compartilhado com o card em `updates.svelte.ts`).

**0.6.1:**
- **Capas nas playlists de outras pessoas.** Com Client ID próprio (Development Mode), o Spotify recusa
  (403) a leitura da playlist e também o `GET /tracks?ids=` das capas. Agora a ordem é: o token do client
  oficial (o de reprodução), depois a página pública da playlist, com as capas pela prévia (oEmbed) de
  cada música, 16 em paralelo e guardadas na memória enquanto o app estiver aberto. Sem conta, o mesmo
  caminho.
- **Foto e nome nas playlists do GlobalMusic.** A foto é recortada e reduzida para 512 px no app
  (`image.ts`, o mesmo código da foto de perfil) e guardada como data URL junto da playlist. Para tirar a
  foto há um campo próprio (`removeCover`), porque em JSON um `null` não diferencia "não mexer" de "sem foto".
- **Compartilhar.** O servidor guarda uma cópia da playlist (migração `0003`) e devolve
  `https://api.globalmusic.216saint.com.br/p/<código>`. Colado em "Importar por link", o link vira uma
  cópia na biblioteca de quem recebeu. Aberto no navegador, mostra uma página com as músicas e o link
  de download. Compartilhar exige a conta GlobalMusic, para o servidor não virar depósito anônimo (limite
  de 300 por pessoa). Playlists de serviço compartilham o próprio link do Spotify, YouTube Music ou SoundCloud.
- **Busca acha vídeos.** A busca do YouTube Music só usava o filtro "Músicas", e muita coisa só existe
  como vídeo enviado por alguém (versões slowed, mashups, remixes de fã). Agora "Músicas" e "Vídeos" são
  buscados ao mesmo tempo e intercalados, sem repetir. Nos vídeos, o "artista" é o canal
  (`MUSIC_PAGE_TYPE_USER_CHANNEL`).
- **Lista atualizada ao voltar para o app.** A playlist aberta era lida só uma vez: curtir algo no site do
  YouTube Music não aparecia. Ao recuperar o foco (depois de 20 s), a playlist aberta e a biblioteca são
  lidas de novo, sem piscar a tela.
- **Botão direito.** O menu padrão da WebView (Voltar, Atualizar, Imprimir, Compartilhar com o Windows)
  foi desligado, exceto em campos de texto. Playlists ganharam menu próprio: tocar, abrir, editar,
  compartilhar, apagar e tirar da biblioteca.

**Instalador com a cara do GlobalMusic** (`bundle.windows.nsis` no `tauri.conf.json`):
- Painel lateral das telas de boas-vindas e de conclusão (`src-tauri/installer/sidebar.bmp`, 164×314) e
  cabeçalho das outras telas (`header.bmp`, 150×57). O NSIS só aceita BMP nesses tamanhos. As imagens foram
  desenhadas por código, com as mesmas barras de `Bars.svelte`.
- Instalador em português (`PortugueseBR`), ícone do app e editor/copyright ("GlobalMusic"), que aparecem
  em "Aplicativos instalados" do Windows.

**0.6.2: o primeiro teste num PC de outra pessoa.** Um colega, com Spotify Free, achou três problemas
que no PC do desenvolvedor não apareciam:
- **Toda faixa do YouTube pulava** com `yt-dlp: Requested format is not available`. Logado, o yt-dlp usa
  clientes do YouTube que exigem um runtime JavaScript (Node, Deno…) para liberar os formatos de áudio.
  O PC do desenvolvedor tem Node instalado, o do colega não. Sem login, os clientes usados não precisam
  disso (reproduzido aqui escondendo o Node do PATH). Agora, nesse erro, o app tenta de novo sem os cookies
  e segue assim até fechar (`without_cookies` em `audio/ytdlp.rs`).
  *Lição:* testar com o PATH de um PC "limpo"; o que o desenvolvedor tem instalado esconde dependências.
- **Biblioteca vazia com "muitas requisições"** (429). Sem Client ID próprio, todo mundo divide a cota do
  client oficial do Spotify. Agora a última biblioteca lida fica em disco (`spotify/cache.rs`) e aparece
  quando o Spotify limita. O erro também passou a levar `retryIn` (os segundos do `Retry-After`), e a
  interface tenta de novo sozinha quando o prazo acaba, em vez de pedir para a pessoa tentar.
- **Playlist importada não abria.** O fallback para a página pública da playlist só acontecia com 403.
  Agora o 429 também cai nele, e a página pública não gasta a cota da Web API.

**Discord:** o ícone ao lado de "Ouvindo GlobalMusic" é decidido pelo próprio Discord. O app já tem ícone
no Developer Portal (conferido pela API pública), e o protocolo do Rich Presence não tem campo para isso.

**Sincronizar o perfil:** o servidor guarda nome e foto de cada login vinculado (migração `0002`, com backup
do banco antes) e oferece `POST /me/profile-from`. A pessoa escolhe "usar o perfil do Discord/Spotify".

**Interface:**
- A fila virou menu lateral (o player continua visível e se recentraliza).
- Capas próprias para as "Curtidas" de cada serviço.
- Cards de tamanho fixo: o `aspect-ratio` sozinho não segura uma foto alta num item flex, por causa do
  mínimo automático; foi preciso `min-height: 0` e a imagem absoluta.
- Logo só com as barras (SVG, sem o fundo do PNG).
- Barras de rolagem do tema.
- Sem os textos "Spotify Free: áudio vindo do YouTube".

### Estado ao fim da sessão

- ✅ 65 testes passando (+1 manual), zero avisos, sem `allow(dead_code)`.
- ✅ Fase 2: busca, biblioteca e faixas reais nos três serviços.
- ✅ Fase 3: áudio (librespot, yt-dlp + rodio, SoundCloud MP3/HLS), fim de faixa, falhas, fallback Free.
- ✅ Login dos três serviços, sem nenhuma configuração.
- ✅ Discord: app criado, ID em `config.rs` e Art Assets enviados.
- ⏳ Primeiro teste real com contas (ver "Como testar" no README).

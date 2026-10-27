# 0008 — Servidor social: perfis, amizades e Party

**Status:** aprovada (2026-10-26). S1 e S2 no ar; S3 (Party) a seguir.

## Por que precisa de um servidor

Até a 0.1.3, cada GlobalMusic instalado é uma ilha: fala direto com Spotify, YouTube e SoundCloud
e não sabe que outras pessoas existem. "Meu perfil", "meus amigos" e "a Party em que estou" são
informações **compartilhadas** entre instalações. Alguém precisa guardá-las e repassar em tempo real
o que o host da Party faz. Esse alguém é um servidor.

## Decisões do dono do projeto

- **Login:** com **Discord** ou com **Spotify** (qualquer um dos dois; dá para ligar o outro depois).
- **Party:** **cada membro toca a música no próprio app**; o servidor só repassa comandos, não áudio.

## Desenho proposto (decisões do assistente, para revisão)

### Onde e com o quê

| Peça | Escolha | Por quê |
|---|---|---|
| Endereço | `https://api.globalmusic.216saint.com.br` | Mesmo padrão do `globalmusic.` (Cloudflare DNS → Caddy na VPS) |
| Linguagem | **Rust** (axum + tokio) | A mesma do app: um idioma só no projeto, binário pequeno e pouca RAM na VPS |
| Banco | **SQLite** (um arquivo) | Escala de turma; sem servidor de banco para manter; backup = copiar um arquivo |
| Tempo real | **WebSocket** | Conexão aberta nos dois sentidos: o host age e os membros recebem na hora |
| Processo | serviço `systemd` atrás do Caddy | Como os outros sites da VPS |

### 1. Contas e login

```
app ──"Entrar com Discord"──► navegador ──► Discord ──► servidor (troca o código) ──► 127.0.0.1:47823 (app recebe a sessão)
app ──"Entrar com Spotify"──► manda o token do Spotify que já tem ──► servidor confere em /v1/me ──► sessão
```

- **Discord:** OAuth2 padrão. Usa o app "GlobalMusic" que já existe no Developer Portal (o mesmo do
  Rich Presence): basta gerar o *client secret* e cadastrar o redirect do servidor. O secret fica
  **só no servidor**.
- **Spotify:** o app já está logado no Spotify. Ele manda o token ao servidor, que pergunta ao próprio
  Spotify "de quem é este token?" (`GET /v1/me`). Isso evita um segundo app no Spotify (com o
  "development mode") só para identidade.
- **Sessão:** token aleatório de 256 bits, guardado no cofre do SO do lado do app. No servidor, só o
  **hash** (vazar o banco não vaza sessões). Expira em 90 dias, renovado com o uso.
- **Contas vinculadas:** a mesma pessoa pode entrar pelos dois. Na primeira vez, a conta é criada; o
  outro login pode ser vinculado no Perfil.

### 2. Perfil

- Nome de exibição, **@usuário** único (para ser achado por amigos) e **foto**.
- Foto inicial: a do Discord/Spotify. Upload próprio: PNG/JPEG/WebP até 2 MB. O servidor
  **decodifica e reencoda** a imagem em 256×256 WebP, o que descarta metadados (como localização
  da câmera) e neutraliza arquivos maliciosos disfarçados de imagem.
- Status visível para amigos: online/offline e "ouvindo agora" (opcional, desligável).

### 3. Amizades

- Adicionar por **@usuário**: pedido → aceitar/recusar. Bloquear também.
- Lista de amigos com status ao vivo (pelo mesmo WebSocket).
- Privacidade: só amigos veem o "ouvindo agora"; ninguém vê suas contas conectadas.

### 4. Party

- O host cria a Party (código de 6 letras) e convida amigos (ou compartilha o código).
- **O servidor guarda o estado oficial:** fila, faixa atual, se está pausada e o **instante em que a
  faixa começou**, no relógio do servidor.
- **Sincronia:** cada app mede a diferença entre o próprio relógio e o do servidor (ping de ida e
  volta, como o NTP faz) e calcula a posição certa. Se um membro ficar mais de 2 s adiantado ou
  atrasado, ele faz seek sozinho.
- **Cada um toca pela própria fonte:** a faixa é a mesma, mas o caminho do áudio é o de cada membro
  (Spotify Premium, YouTube para Free etc.). Quem não conseguir tocar uma faixa vê o aviso, e a
  Party segue.
- **Permissões:** o host controla play/pausa/pular/fila. Por membro, ele pode liberar
  "pode adicionar à fila". Membros sem permissão podem **sugerir**, e o host aceita.
- Se o host sair, a Party passa para o membro mais antigo (ou acaba, se o host preferir).

### Segurança (o mínimo inegociável)

- Tudo em HTTPS/WSS; secrets só no servidor (`/etc/globalmusic/api.env`, permissão 600).
- Limite de requisições por IP e por conta; tamanho máximo de mensagem no WebSocket.
- Upload: tipo verificado pelo conteúdo (não pela extensão), reencode obrigatório, limite de tamanho.
- Toda ação de Party confere no servidor se quem pediu tem permissão (o app nunca é a autoridade).
- Backup diário do SQLite.

## Fases

1. **S1 — Contas e perfil:** servidor no ar, login Discord + Spotify, perfil com foto.
2. **S2 — Amizades:** pedidos, lista, status ao vivo.
3. **S3 — Party:** salas, sincronia, permissões, fila compartilhada.

Cada fase sai como uma versão do app (pelo atualizador), com o servidor publicado antes.

## O que o dono do projeto precisa fazer

- No Discord Developer Portal (app GlobalMusic): **OAuth2 → gerar Client Secret** e adicionar o redirect
  `https://api.globalmusic.216saint.com.br/auth/discord/callback`. O assistente cria o DNS e o
  serviço na VPS.

## Implantação (S1 + S2)

| O quê | Onde |
|---|---|
| Código | `server/` (este repositório); cópia de build em `/home/ubuntu/projetos/globalmusic-api` |
| Binário | `/opt/globalmusic-api/globalmusic-api` |
| Serviço | `globalmusic-api.service` (usuário `globalmusic`, porta local 3480) |
| Configuração | `/etc/globalmusic/api.env` (modo 640, root:globalmusic) |
| Banco e fotos | `/var/lib/globalmusic/api.db` e `/var/lib/globalmusic/avatars/` |
| HTTPS | Caddy, site `api.globalmusic.216saint.com.br` (`/avatars/*` direto do disco) |
| DNS | Cloudflare, registro A só DNS |

Atualizar o servidor: copiar `server/` para a VPS, `cargo build --release`, copiar o binário para
`/opt/globalmusic-api/` e `sudo systemctl restart globalmusic-api`.

Login pelo Discord: liga sozinho quando `DISCORD_CLIENT_SECRET` é preenchido no `api.env` e o serviço
reinicia. Não precisa de versão nova do app.

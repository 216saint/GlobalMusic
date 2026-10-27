# O fluxo completo: do código ao app atualizado no PC de alguém

Este texto é o mapa geral, escrito para quem vem de web apps. Os detalhes de cada etapa estão
nos textos linkados.

## 1. O que torna o GlobalMusic um "app"?

Num **web app**, o seu código roda em dois lugares que você controla ou que sempre existem: o
**servidor** (a sua VPS) e o **navegador** da pessoa, que baixa HTML/JS a cada visita.

No **app desktop**, o código vira um **programa instalado no computador da pessoa**:

| | Web app | GlobalMusic |
|---|---|---|
| Onde o código roda | Seu servidor + navegador | Um processo `globalmusic.exe` na máquina da pessoa |
| Linguagem do "backend" | Node/PHP… no servidor | **Rust**, compilado para código de máquina, dentro do próprio app |
| Interface | HTML/CSS/JS no navegador | HTML/CSS/JS (Svelte) numa **WebView**: um "navegador sem barras" embutido na janela |
| Quem fala com Spotify/YouTube | Seu servidor | O **próprio app**, direto do PC da pessoa (sem servidor no meio) |
| Acesso ao sistema | Quase nenhum (sandbox do navegador) | Placa de som, cofre de senhas do SO, processos (yt-dlp), janelas, Discord |
| Dados do usuário | Seu banco de dados | No PC da pessoa (tokens no cofre do SO, cookies no perfil da WebView) |
| Entrega | Uma URL | Um **instalador** (`.exe`, `.dmg`, `.deb`) |
| Atualização | Deploy: todos recebem na hora | O app **baixa e instala** a versão nova (updater) |

A frase-chave: **o app é o servidor e o cliente ao mesmo tempo, rodando na máquina de cada
pessoa.** A sua VPS só serve os arquivos de instalação e atualização.

### Por dentro: dois processos conversando

```
┌─ globalmusic.exe (Rust) ──────────────────────────────┐      ┌─ WebView (msedgewebview2) ─┐
│ comandos (search, play…), áudio, logins, Discord      │◄────►│ interface Svelte            │
│ fala com Spotify / YouTube / SoundCloud pela internet │ IPC  │ (só desenha e pede coisas)  │
└───────────────────────────────────────────────────────┘      └─────────────────────────────┘
```

O "IPC" faz o papel que o `fetch('/api/...')` faria num web app, só que entre dois processos na
mesma máquina, sem rede. Ver [ipc-tauri.md](ipc-tauri.md).

## 2. As etapas, lado a lado

| Etapa | Web app | GlobalMusic |
|---|---|---|
| **Desenvolver** | `npm run dev` + navegador | `npm run app:dev`: compila o Rust e abre a janela do app, com recarga ao salvar |
| **Testar** | testes JS | `npm run test:rust` (lógica) + `npm run check` (tipos da UI) |
| **Build** | `vite build` → pasta `dist/` | `vite build` → `dist/` **+** `cargo build --release` → `globalmusic.exe` |
| **Empacotar** | Imagem Docker / pasta | **Bundler** do Tauri: põe o `.exe`, o yt-dlp e os ícones num **instalador** (NSIS) |
| **Assinar** | HTTPS no servidor | O instalador é **assinado** com a sua chave privada (`.sig`) para o updater confiar nele |
| **Publicar** | Deploy na VPS | Envia o instalador + `latest.json` para a VPS (só arquivos estáticos) |
| **Rodar** | O servidor fica ligado | Cada pessoa abre o app no próprio PC; a VPS pode até estar desligada |
| **Atualizar** | Novo deploy, efeito imediato | O app de cada pessoa **verifica**, mostra "Atualização disponível" e se reinstala |

O que mais muda na prática:

1. **Compilar demora** (minutos), porque o Rust gera código de máquina otimizado. No web, o build é quase instantâneo.
2. **Um build por sistema operacional.** O `.exe` do Windows não roda no Mac. Ver [distribuicao.md](distribuicao.md).
3. **Não dá para "consertar no servidor".** Um bug fica na máquina das pessoas até elas atualizarem. Por isso o updater é essencial.
4. **Confiança:** sistemas desconfiam de programas baixados (SmartScreen, Gatekeeper). No web, quem dá essa garantia é o HTTPS.

## 3. CI/CD: o que é e como aparece aqui

- **CI (Integração Contínua):** a cada mudança, uma máquina **automática** compila e testa o
  projeto. O objetivo é descobrir cedo que algo quebrou, sem depender de lembrar de rodar os testes.
- **CD (Entrega/Deploy Contínuo):** a mesma automação **publica** o resultado (gera os
  instaladores e coloca onde as pessoas baixam).

No GlobalMusic há dois caminhos de CD:

```
CAMINHO ATUAL (manual, do seu PC)                     CAMINHO CI (GitHub Actions, preparado)
───────────────────────────────────                    ───────────────────────────────────────
npm run release -- patch --notes "…"                   git tag v0.2.0 && git push --tags
 ├─ sobe a versão (package.json)                         │
 ├─ compila + assina (só Windows)                        ├─ GitHub liga 3 máquinas: Windows, macOS, Ubuntu
 ├─ gera latest.json                                     ├─ cada uma: npm ci → yt-dlp → tauri build
 └─ scp → VPS /srv/globalmusic                           └─ anexa os instaladores numa Release (rascunho)
          │
          ▼
https://globalmusic.216saint.com.br/latest.json
          │
          ▼
apps instalados: "Atualização disponível" → baixa → confere assinatura → instala → reabre
```

- O caminho **atual** é o `scripts/release.mjs` ([ADR 0007](../decisoes/0007-atualizacao-automatica.md)): você
  roda um comando e ele faz o resto.
- O caminho **CI** é o `.github/workflows/release.yml`, pronto para quando o código estiver no
  GitHub. A vantagem dele é gerar os instaladores dos três sistemas sem você ter um Mac ou um Linux.
- Um passo natural de evolução é juntar os dois: o CI compila os três sistemas, assina com a chave
  guardada como *secret* do GitHub e envia tudo para a VPS. Aí, publicar uma versão vira só
  "criar uma tag".

## 4. Onde está cada coisa

| Quero entender… | Leia |
|---|---|
| quem fala com quem dentro do app | [../arquitetura.md](../arquitetura.md) |
| instaladores, assinatura de código, um build por sistema | [distribuicao.md](distribuicao.md) |
| atualização automática e a VPS | [../decisoes/0007-atualizacao-automatica.md](../decisoes/0007-atualizacao-automatica.md) |
| como frontend e Rust conversam | [ipc-tauri.md](ipc-tauri.md) |
| o que deu errado e como foi resolvido | [../diario.md](../diario.md) |

<div align="center">

<img src="docs/assets/logo.png" alt="Logo do GlobalMusic" width="96" height="96">

# GlobalMusic

**Spotify, YouTube Music e SoundCloud num app só, consumindo menos de 50mb de RAM.**

Busca procura nas três fontes ao mesmo tempo, a biblioteca junta as playlists de todas as
suas contas e a fila mistura faixas de qualquer lugar.

<a href="https://globalmusic.216saint.com.br/GlobalMusic_0.6.0_x64-setup.exe"><img alt="Baixar para Windows" src="https://img.shields.io/badge/Baixar-Windows%2010%2B-7b6cff?style=for-the-badge&logo=windows&logoColor=white"></a>
<img alt="RAM abaixo de 150 MB" src="https://img.shields.io/badge/RAM-%3C%20150%20MB-1ed760?style=for-the-badge">
<img alt="Feito com Tauri e Rust" src="https://img.shields.io/badge/Tauri%202-Rust-f5f4ff?style=for-the-badge&logo=tauri&logoColor=24c8db&labelColor=16151d">

![GlobalMusic tocando uma faixa, com a busca nas três fontes ao lado](docs/assets/screenshot.jpg)

</div>

## 🌃 Recursos

- 🔎 **Busca federada**: Spotify, YouTube Music e SoundCloud de uma vez, com resultados intercalados.
- 📚 **Biblioteca unificada**: as playlists de todas as contas, com o selo da fonte no canto de cada capa.
- 🎶 **Fila multiplataforma**, como a do Spotify: o que você adiciona toca antes do resto da playlist.
- 🆓 **Spotify Free toca**: a mesma gravação é encontrada no YouTube e tocada de lá.
- 🔁 **Escolha a versão**: se a música do Spotify cair numa versão errada do YouTube (ao vivo, cover), você troca e o app lembra.
- 🎤 **Letras sincronizadas** pelo [LRCLIB](https://lrclib.net): o verso do momento fica destacado e clicar num verso pula até ele.
- ⏭️ **Pula trechos sem música** dos clipes do YouTube (introduções faladas, cenas depois da música), marcados pela comunidade do [SponsorBlock](https://sponsor.ajay.app). A consulta é anônima.
- ⚡ **Começa rápido**: as faixas em tela são preparadas antes do clique, a próxima da fila já fica baixada e as que você já ouviu tocam do cache em disco.
- 🪶 **Leve**: menos de **150 MB de RAM** (o app oficial do Spotify passa de 600 MB), instalador de ~18 MB e cache de áudio limitado a 300 MB.
- 👥 **Perfil e amigos**: entre com Discord ou Spotify, coloque foto e @usuário e adicione amigos.
- 🎮 **Discord Rich Presence**: "Ouvindo GlobalMusic" com capa, artista e tempo.
- 🔄 **Atualização automática**, com o pacote assinado e conferido antes de instalar.
- 🔒 **Sem telemetria**: os tokens ficam no cofre do sistema, e ninguém precisa criar chave de desenvolvedor.
- 🧊 **Glassmorphism** com o vidro nativo do sistema (Acrylic no Windows, Vibrancy no macOS).

## ⬇️ Download

| Sistema | Como instalar |
|---|---|
| **Windows 10+** | [Baixe o instalador](https://globalmusic.216saint.com.br/GlobalMusic_0.6.0_x64-setup.exe) (o Windows pode avisar que o app não é assinado: *Mais informações → Executar assim mesmo*) |
| **macOS Tahoe+** | Por enquanto, [compile do código](#️-compilando-do-código) |
| **Ubuntu 22.04+** | Por enquanto, [compile do código](#️-compilando-do-código) |

Depois de instalado, o app se atualiza sozinho: quando sai uma versão nova, aparece o aviso
**"Atualização disponível"**.

### Conectando as contas

Ninguém configura nada: cada pessoa entra com a própria conta pela tela **Perfil**.

| Fonte | Como é o login |
|---|---|
| Spotify | No navegador (contas Free e Premium) |
| YouTube Music | Na página oficial do Google, numa janela do app |
| SoundCloud | Na página oficial do SoundCloud, numa janela do app |

## 🧭 Como funciona

Cada faixa segue uma rota, decidida em [`player/routing.rs`](src-tauri/src/player/routing.rs):

| Faixa | De onde vem o áudio |
|---|---|
| YouTube Music | YouTube, resolvido pelo [yt-dlp](https://github.com/yt-dlp/yt-dlp) |
| SoundCloud | API do SoundCloud (MP3 ou AAC) |
| Spotify | Spotify Premium pelo [librespot](https://github.com/librespot-org/librespot); senão, a mesma gravação no YouTube |

Todo o áudio é decodificado no processo Rust e a interface nunca toca mídia, por isso a RAM fica
baixa. Os detalhes estão em [docs/arquitetura.md](docs/arquitetura.md).

| Camada | Tecnologia |
|---|---|
| App | [Tauri 2](https://tauri.app) (Rust), com a WebView do próprio sistema |
| Interface | [Svelte 5](https://svelte.dev) + Vite + TypeScript |
| Áudio | `rodio` + `symphonia`, `librespot` (Spotify Premium), `yt-dlp` (YouTube) |
| Contas sociais | Servidor próprio em Rust ([`server/`](server/)): axum + SQLite |

## 🕳️ Compilando do código

### Pré-requisitos

- **Node 20+** e **Rust** estável (`rustup`).
- **Windows**: Build Tools do Visual Studio (C++) e WebView2 (já vem no Windows 11).
- **macOS**: Xcode Command Line Tools (`xcode-select --install`).
- **Ubuntu 22.04+**:

  ```sh
  sudo apt install build-essential pkg-config libwebkit2gtk-4.1-dev libssl-dev \
    libasound2-dev libayatana-appindicator3-dev librsvg2-dev libdbus-1-dev
  ```

### Rodando

```sh
npm install
npm run setup:yt-dlp        # baixa o yt-dlp oficial e confere o SHA-256
npm run app:dev             # abre o app em modo desenvolvimento
```

Só a interface, no navegador e com dados de demonstração: `npm run dev`.

### Comandos

| Comando | O que faz |
|---|---|
| `npm run app:build` | Gera os instaladores do sistema atual, com o yt-dlp embutido |
| `npm run app:build:win` | Só o instalador `.exe` do Windows |
| `npm run test:rust` | Testes do backend |
| `npm run check` | Checagem de tipos da interface |
| `npm run measure:ram` | Soma a RAM do app e dos processos da WebView |
| `npm run release -- patch --notes "…"` | Publica uma versão nova para o atualizador ([ADR 0007](docs/decisoes/0007-atualizacao-automatica.md)) |

### Estrutura

```text
src/                     interface (Svelte 5)
src-tauri/src/
  commands.rs            comandos que a interface chama
  aggregate.rs           busca e biblioteca nas três fontes ao mesmo tempo
  providers/             Spotify, YouTube Music e SoundCloud
  player/                fila, rotas e equivalência no YouTube
  audio/                 download, cache em disco, yt-dlp, librespot e saída de som
  lyrics.rs              letras (LRCLIB)
  sponsorblock.rs        trechos sem música (SponsorBlock)
  social.rs              conta GlobalMusic e amigos
server/                  servidor social (contas, fotos, amizades)
docs/                    documentação de estudo
```

<div align="center"><h4>© GlobalMusic 2026</h4></div>

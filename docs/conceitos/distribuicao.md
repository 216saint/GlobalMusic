# Distribuição: como um app desktop chega no computador das pessoas

## Web app × app desktop

| | Web app (o que você já conhece) | App desktop (o GlobalMusic) |
|---|---|---|
| Onde o código roda | No **seu servidor** (VPS) e no navegador | No **computador de cada usuário** |
| O que você entrega | Uma URL | Um **arquivo de instalação** |
| Precisa de servidor? | Sim, sempre ligado | **Não**: o app fala direto com Spotify, YouTube e SoundCloud a partir da máquina da pessoa |
| Um build serve para todos? | Sim (o navegador interpreta) | **Não**: um build por sistema operacional (e arquitetura) |
| Atualizar | Você faz deploy e todos recebem na hora | Cada pessoa instala a versão nova (ou o app se atualiza sozinho, ver abaixo) |
| Custo de hospedagem | VPS mensal | Zero: o instalador fica no **GitHub Releases** (grátis) |

A mudança de mentalidade: **não existe "o servidor do GlobalMusic"**. Cada instalação é
independente. A sua conta do Spotify só existe no seu computador (no cofre de credenciais do
seu SO), e a de um colega, só no dele.

## Por que um build por sistema?

O Rust compila para **código de máquina** do processador e do sistema alvo. Um `.exe` do
Windows não roda no Mac, assim como um binário de Linux não roda no Windows. Por isso o
Tauri gera um instalador por plataforma:

| Sistema | Arquivo | Como a pessoa instala |
|---|---|---|
| Windows | `GlobalMusic_0.1.0_x64-setup.exe` (NSIS) | Duplo clique, avançar, pronto. Instala no usuário, sem pedir admin |
| Windows | `GlobalMusic_0.1.0_x64_en-US.msi` | Alternativa para instalação gerenciada (empresas) |
| macOS | `GlobalMusic_0.1.0_universal.dmg` | Abre o `.dmg` e arrasta para Aplicativos (roda em Intel e Apple Silicon) |
| Ubuntu | `globalmusic_0.1.0_amd64.deb` | Duplo clique ou `sudo apt install ./globalmusic_*.deb` |
| Linux | `globalmusic_0.1.0_amd64.AppImage` | Arquivo único: dá permissão de execução e roda |

E um detalhe importante: **cada sistema só gera o próprio instalador**. O seu Windows gera os
dois de Windows, mas não consegue gerar o `.dmg`.

## Como gerar os três sem ter três computadores: GitHub Actions

O arquivo `.github/workflows/release.yml` pede ao GitHub que **alugue três máquinas virtuais**
(Windows, macOS e Ubuntu), compile o app em cada uma e anexe os instaladores numa
*Release*, a página de downloads do repositório.

```
você: git tag v0.1.0 && git push --tags
                │
   ┌────────────┼────────────┐
   ▼            ▼            ▼
windows      macos        ubuntu        ← 3 máquinas do GitHub, em paralelo
   │            │            │
 .exe/.msi     .dmg      .deb/.AppImage
   └────────────┼────────────┘
                ▼
     GitHub Release (rascunho)  →  você revisa e publica  →  link para a turma
```

Para repositórios públicos isso é gratuito. Em privados, existe uma cota mensal de minutos.

## "O Windows protegeu o computador": assinatura de código

Sistemas operacionais desconfiam de programas baixados da internet sem uma **assinatura
digital**, um certificado que prova quem publicou o arquivo. Esse certificado é pago
(centenas de dólares por ano), então o GlobalMusic sai **sem assinatura**, e cada sistema
reclama de um jeito:

- **Windows (SmartScreen):** "O Windows protegeu o computador" → *Mais informações* → *Executar assim mesmo*.
- **macOS (Gatekeeper):** "não pode ser aberto". Vá em *Ajustes do Sistema → Privacidade e
  Segurança* e clique em *Abrir mesmo assim*. Se aparecer "está danificado", rode
  `xattr -cr /Applications/GlobalMusic.app` no Terminal. Usamos assinatura *ad-hoc*
  (`signingIdentity: "-"`), que é o mínimo exigido no Apple Silicon.
- **Linux:** não reclama.

## Onde fica cada coisa depois de instalado (Windows)

| O quê | Onde |
|---|---|
| Programa | `%LOCALAPPDATA%\GlobalMusic\` |
| Dados (cache do client ID do Spotify, perfil da WebView com os cookies de login) | `%LOCALAPPDATA%\com.globalmusic.app\` |
| Token do Spotify | Gerenciador de Credenciais do Windows (`com.globalmusic.app`) |
| Desinstalar | Configurações → Aplicativos → GlobalMusic |

## Atualização automática

Depois da primeira instalação, ninguém precisa reinstalar. O app verifica na sua VPS se há
versão nova e mostra o card "Atualização disponível". As atualizações são **assinadas** com
uma chave sua, para ninguém conseguir empurrar uma "atualização" falsa. Detalhes e
configuração da VPS em [ADR 0007](../decisoes/0007-atualizacao-automatica.md).

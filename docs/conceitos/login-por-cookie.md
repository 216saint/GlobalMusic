# Login por cookie e SAPISIDHASH

## A ideia

Quando você entra no music.youtube.com pelo navegador, o site guarda **cookies de
sessão**. Daí em diante, toda requisição que o site faz leva esses cookies, e é assim que
o servidor sabe que é você. Não existe "app registrado" nessa história: é só o site
falando com o próprio servidor.

O GlobalMusic faz a mesma coisa (`auth/web_login.rs`):

1. abre a **página oficial de login** numa janela do app;
2. a cada 0,8 s, verifica se o **cookie de sessão** apareceu (`SAPISID` no Google,
   `oauth_token` no SoundCloud);
3. quando aparece, fecha a janela. Login feito.

Depois, o backend Rust lê esses cookies (`cookies_for_url`) e faz as mesmas requisições
que o site faria.

### Por que verificar o cookie e não a URL?

Verificar "a página mudou para music.youtube.com?" parece mais simples, mas o login do
Google tem dezenas de caminhos (2FA, escolha de conta, captcha, "confirme que é você").
O cookie de sessão é o **sinal que importa**: ou existe, ou não existe.

### Onde os cookies ficam guardados?

No perfil da WebView do sistema, igual a um navegador, e o SO os criptografa. Todas as
janelas do app compartilham esse perfil, por isso `any_window` serve para ler os cookies
de qualquer janela.

### Armadilha do Windows

`cookies_for_url` **trava o app** se for chamado na thread principal (bug conhecido do
WebView2). Por isso só é chamado a partir de comandos `async`. O comentário no código
avisa, porque esse é o tipo de coisa que você esquece em seis meses.

## SAPISIDHASH: a assinatura do YouTube

Requisições logadas ao YouTube precisam de um header extra:

```
Authorization: SAPISIDHASH 1700000000_9ad4ca9e...
                           └─ timestamp └─ sha1("1700000000 <SAPISID> https://music.youtube.com")
```

É uma prova de que quem está pedindo conhece o cookie `SAPISID` **e** está falando a
partir da origem certa. A implementação tem 3 linhas (`youtube_music.rs` → `sapisid_hash`).

**Como testar algo assim?** Calculamos a resposta esperada por **outro caminho**, o
`crypto` do Node, e escrevemos no teste. Se o Rust concordar com uma implementação
independente, temos confiança. O comando usado está no comentário do teste.

## SoundCloud: o client_id público

A API do site do SoundCloud exige um `client_id`. Ele não é secreto: o próprio site o
embute nos arquivos JavaScript que todo visitante baixa. `soundcloud.rs` faz o que o seu
navegador faz:

1. baixa `soundcloud.com`;
2. acha os `<script src="https://a-v2.sndcdn.com/assets/...js">` (`script_urls`);
3. procura `client_id:"<32 caracteres>"` neles (`client_id_in`).

As duas funções são **puras**: recebem texto e devolvem texto, sem rede. Por isso são
fáceis de testar com HTML e JS de exemplo.

## O risco

APIs internas não são documentadas e podem mudar sem aviso. É por isso que:

- cada serviço está isolado no seu provedor;
- a busca federada tolera uma fonte quebrada (ver [async-fan-out](async-fan-out.md));
- as partes frágeis (parsers) são funções pequenas com teste, fáceis de consertar.

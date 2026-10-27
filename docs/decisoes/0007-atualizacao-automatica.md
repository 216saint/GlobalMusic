# 0007 — Atualização automática pela VPS

**Status:** aceita (2026-10-26)

## Contexto

Cada correção exigia gerar um instalador novo e pedir para cada pessoa reinstalar. O pedido:
o app deve mostrar **"Atualização disponível"** e se atualizar sozinho.

## Alternativas de hospedagem

| Opção | Prós | Contras |
|---|---|---|
| GitHub Releases (código privado + repositório público de releases) | Grátis, e o CI compila os 3 sistemas | Exige conta e dois repositórios |
| GitHub, tudo público | O mais simples | O código fica visível |
| **VPS do dono do projeto** (escolhida) | Controle total, sem depender de terceiros, código fica privado | Publicação manual (`npm run release`), e por enquanto só o Windows recebe atualizações automáticas |

## Como funciona

```
  seu PC                                   VPS (https)                     app instalado
  npm run release -- patch --notes "…"
   ├─ versão 0.1.0 → 0.1.1
   ├─ compila + ASSINA com a chave privada
   ├─ gera latest.json  ───── scp ──►  /globalmusic/latest.json  ◄── verifica ao abrir e a cada 6 h
   └─ instalador .exe    ───── scp ──►  /globalmusic/GlobalMusic_0.1.1_x64-setup.exe
                                                                    ├─ "Atualização disponível 0.1.0 → 0.1.1"
                                                                    ├─ baixa e CONFERE a assinatura
                                                                    └─ instala e reabre sozinho
```

### `latest.json`

```json
{
  "version": "0.1.1",
  "notes": "Correção no login do SoundCloud",
  "pub_date": "2026-10-27T12:00:00.000Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "<conteúdo do .sig>",
      "url": "https://seudominio.com/globalmusic/GlobalMusic_0.1.1_x64-setup.exe"
    }
  }
}
```

O app compara a `version` com a sua. Se for maior, mostra o card com as `notes`.

### Assinatura: por que ela é obrigatória

Sem assinatura, quem conseguisse alterar o `latest.json` (invadindo a VPS, por exemplo)
poderia fazer **todos os apps instalados** baixarem e executarem um programa qualquer. Por isso:

- **Chave privada** (`C:\Users\looks\.tauri\globalmusic.key`): só no seu computador. Ela assina
  cada instalador e gera o `.sig`.
- **Chave pública:** vai embutida no app (`tauri.conf.json → plugins.updater.pubkey`). Antes de
  instalar, o app confere a assinatura e **recusa** o que não foi assinado pela sua chave privada.

> ⚠️ **Faça backup da chave privada** (num pen drive ou gerenciador de senhas). Se ela for
> perdida, os apps já instalados nunca mais aceitam atualizações, e todo mundo teria que
> reinstalar manualmente uma versão com uma chave nova.
> Ela **nunca** vai para o git (fica fora da pasta do projeto).

### Peças no código

| Peça | Onde |
|---|---|
| Plugin + chave pública + URL | `src-tauri/tauri.conf.json → plugins.updater` |
| Gerar instalador assinado | `bundle.createUpdaterArtifacts: true`; `scripts/with-cargo.mjs` aponta a chave |
| Verificar e instalar | `src-tauri/src/updates.rs` (`update_check`, `update_install`) |
| Card "Atualização disponível" | `src/lib/components/UpdateCard.svelte` |
| Publicar | `scripts/release.mjs` (`npm run release`) |
| Versão única | `package.json` (o `tauri.conf.json` aponta para ele com `"version": "../package.json"`) |

Detalhes que valem a pena notar:

- **Ordem do upload:** o instalador sobe primeiro e o `latest.json` por último. Se fosse ao
  contrário, um app poderia ver a versão nova antes de o instalador existir na VPS.
- **Verificação silenciosa:** se a VPS estiver fora do ar, o app não mostra erro. Só mostra o
  card quando há de fato uma versão nova.
- **Windows:** o instalador roda em modo `passive` (só uma barra de progresso, sem perguntas),
  fecha o app e o reabre na versão nova.
- **`--dry-run`:** gera o instalador e o `latest.json` em `instaladores/` sem enviar nada. Bom
  para conferir antes de publicar.

## Configurar a VPS (uma vez)

Endereço escolhido: **`https://globalmusic.216saint.com.br/latest.json`**, num subdomínio do
portfólio (`216saint.com.br` → `84.46.242.26`). Já está configurado em
`src-tauri/tauri.conf.json`.

1. **DNS:** no painel onde o domínio é gerenciado, crie um registro
   `A  globalmusic  →  84.46.242.26` (o mesmo IP do portfólio). Confira com
   `Resolve-DnsName globalmusic.216saint.com.br`.

2. **Pasta e nginx** (na VPS):

   ```sh
   sudo mkdir -p /var/www/globalmusic && sudo chown $USER /var/www/globalmusic
   sudo nano /etc/nginx/sites-available/globalmusic
   ```

   ```nginx
   server {
       listen 80;
       server_name globalmusic.216saint.com.br;
       root /var/www/globalmusic;

       # O latest.json não pode ficar em cache, senão o app demora a ver a versão nova.
       location = /latest.json { add_header Cache-Control "no-store"; }
       location / { try_files $uri =404; }
   }
   ```

   ```sh
   sudo ln -s /etc/nginx/sites-available/globalmusic /etc/nginx/sites-enabled/
   sudo nginx -t && sudo systemctl reload nginx
   ```

3. **HTTPS** (o updater recusa HTTP), com Let's Encrypt:

   ```sh
   sudo certbot --nginx -d globalmusic.216saint.com.br
   ```

4. **Destino do upload:** copie `release.config.example.json` para `release.config.json` e ajuste
   o usuário SSH. O upload usa `scp`, que já vem no Windows 10+. Configure login por chave SSH
   (`ssh-keygen` + `ssh-copy-id`) para não digitar senha a cada release.

5. **A primeira instalação é manual:** quem tem a versão antiga (sem updater) precisa instalar
   uma vez a versão com updater (`npm run app:build:win`). Daí em diante, tudo chega pelo card.

Teste do caminho todo: depois do primeiro `npm run release`, abra
`https://globalmusic.216saint.com.br/latest.json` no navegador. Ele deve mostrar a versão nova.

## Consequências

- ✅ Correções chegam a todos sem reinstalar.
- ✅ Atualizações falsificadas são impossíveis sem a chave privada.
- ⚠️ Só o Windows publica por enquanto. macOS/Linux precisariam ser compilados nesses sistemas
  (ou no CI) e entrar no mesmo `latest.json` (`darwin-universal`, `linux-x86_64`).
- ⚠️ A chave privada vira um ativo crítico do projeto (ver aviso acima).

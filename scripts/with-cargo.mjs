// Roda um comando garantindo que o Rust (cargo/rustc) esteja no PATH.
//
// Por quê: terminais (e o VS Code) abertos ANTES de instalar o Rust guardam
// uma cópia antiga do PATH e dão "cargo: program not found", mesmo com o Rust
// instalado. Aqui adicionamos `~/.cargo/bin` (ou `$CARGO_HOME/bin`) se faltar.
// Ver docs/diario.md, problema 1.
//
//   node scripts/with-cargo.mjs tauri dev
//   node scripts/with-cargo.mjs cargo test --manifest-path src-tauri/Cargo.toml
//
// Também exporta `cargoEnv()` para outros scripts (ex.: release.mjs).

import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { delimiter, join } from "node:path";
import { pathToFileURL } from "node:url";

/** Cópia do ambiente atual com o Rust no PATH e a chave de assinatura das atualizações. */
export function cargoEnv() {
  const env = { ...process.env };
  // No Windows a variável pode se chamar "Path"; em outros sistemas, "PATH".
  const pathKey = Object.keys(env).find((k) => k.toUpperCase() === "PATH") ?? "PATH";
  const cargoBin = join(env.CARGO_HOME ?? join(homedir(), ".cargo"), "bin");
  const entries = (env[pathKey] ?? "").split(delimiter);
  if (existsSync(cargoBin) && !entries.includes(cargoBin)) {
    env[pathKey] = [cargoBin, ...entries].join(delimiter);
  }

  // Chave que assina as atualizações (gerada com `tauri signer generate`, fica FORA do repositório).
  // Sem ela, `tauri build` não consegue gerar os artefatos de atualização. Ver docs/decisoes/0007.
  const signingKey = join(homedir(), ".tauri", "globalmusic.key");
  if (!env.TAURI_SIGNING_PRIVATE_KEY && existsSync(signingKey)) {
    env.TAURI_SIGNING_PRIVATE_KEY = signingKey;
    env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ??= "";
  }
  return env;
}

// Executado direto (e não importado): roda o comando recebido.
if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [command, ...args] = process.argv.slice(2);
  if (!command) {
    console.error("uso: node scripts/with-cargo.mjs <comando> [argumentos...]");
    process.exit(2);
  }
  // `shell: true` resolve `tauri.cmd`/`npx.cmd` no Windows e binários do node_modules/.bin.
  const result = spawnSync(command, args, { stdio: "inherit", env: cargoEnv(), shell: true });
  process.exit(result.status ?? 1);
}

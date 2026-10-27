// Publica uma versão nova do GlobalMusic na VPS. Quem já tem o app instalado
// recebe o aviso "Atualização disponível" na próxima vez que ele verificar.
//
//   npm run release -- patch --notes "Correção no login do SoundCloud"
//   npm run release -- minor --notes "Fila com arrastar e soltar"
//   npm run release -- 1.0.0 --notes "..."
//   npm run release -- patch --notes "..." --dry-run    (gera tudo, não envia)
//
// Passos: sobe a versão → compila e ASSINA o instalador → gera latest.json →
// envia o instalador e, por último, o latest.json (via scp).
// Explicação completa: docs/decisoes/0007-atualizacao-automatica.md

import { execFileSync, spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { cargoEnv } from "./with-cargo.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const paths = {
  pkg: join(root, "package.json"),
  cargo: join(root, "src-tauri", "Cargo.toml"),
  tauriConf: join(root, "src-tauri", "tauri.conf.json"),
  config: join(root, "release.config.json"),
  out: join(root, "instaladores"),
};

function fail(message) {
  console.error(`\n✖ ${message}`);
  process.exit(1);
}

// --- argumentos ---
const args = process.argv.slice(2);
const bump = args[0];
const dryRun = args.includes("--dry-run");
// No Windows, o npm repassa os argumentos sem as aspas: `--notes "Correção no login"`
// chega como várias palavras soltas. Então juntamos tudo depois de --notes até a próxima opção.
const notesIndex = args.indexOf("--notes");
const notesEnd = args.findIndex((a, i) => i > notesIndex && a.startsWith("--"));
const notes =
  notesIndex >= 0 ? args.slice(notesIndex + 1, notesEnd === -1 ? undefined : notesEnd).join(" ").trim() || undefined : undefined;
if (!bump || !notes) {
  fail('uso: npm run release -- <patch|minor|major|X.Y.Z> --notes "o que mudou" [--dry-run]');
}

// --- plataforma (por enquanto, publicamos o que este computador compila) ---
const PLATFORMS = {
  win32: { bundle: "nsis", key: "windows-x86_64", file: (v) => `GlobalMusic_${v}_x64-setup.exe` },
};
const platform = PLATFORMS[process.platform];
if (!platform) fail(`publicar a partir de ${process.platform} ainda não é suportado (use o Windows).`);

// --- onde o app procura atualizações (única fonte da verdade: tauri.conf.json) ---
const tauriConf = JSON.parse(readFileSync(paths.tauriConf, "utf8"));
const endpoint = tauriConf.plugins?.updater?.endpoints?.[0] ?? "";
if (!endpoint.startsWith("https://") || endpoint.includes("SEU-DOMINIO")) {
  fail("configure a URL da VPS em src-tauri/tauri.conf.json → plugins.updater.endpoints (ex.: https://globalmusic.216saint.com.br/latest.json).");
}
const baseUrl = endpoint.slice(0, endpoint.lastIndexOf("/"));

// --- destino do upload ---
let target;
if (!dryRun) {
  if (!existsSync(paths.config)) fail("crie release.config.json (copie de release.config.example.json).");
  target = JSON.parse(readFileSync(paths.config, "utf8"));
  if (!target.ssh || !target.remoteDir) fail('release.config.json precisa de "ssh" e "remoteDir".');
}

// --- nova versão ---
const pkgText = readFileSync(paths.pkg, "utf8");
const cargoText = readFileSync(paths.cargo, "utf8");
const current = JSON.parse(pkgText).version;
const next = nextVersion(current, bump);
console.log(`GlobalMusic ${current} → ${next}${dryRun ? " (dry-run)" : ""}`);

function nextVersion(version, how) {
  if (/^\d+\.\d+\.\d+$/.test(how)) return how;
  const [major, minor, patch] = version.split(".").map(Number);
  if (how === "major") return `${major + 1}.0.0`;
  if (how === "minor") return `${major}.${minor + 1}.0`;
  if (how === "patch") return `${major}.${minor}.${patch + 1}`;
  fail(`versão inválida: ${how}`);
}

// O app lê a versão do package.json (tauri.conf.json → "version": "../package.json");
// o Cargo.toml acompanha para manter tudo coerente.
writeFileSync(paths.pkg, pkgText.replace(/"version":\s*"[^"]+"/, `"version": "${next}"`));
writeFileSync(paths.cargo, cargoText.replace(/^version = "[^"]+"/m, `version = "${next}"`));

// --- build assinado ---
const env = cargoEnv();
if (!env.TAURI_SIGNING_PRIVATE_KEY) {
  restoreVersions();
  fail("chave de assinatura não encontrada em ~/.tauri/globalmusic.key (ver docs/decisoes/0007).");
}
// Uma string única (e não lista de argumentos) com `shell: true`: no Windows o `npx` é um
// `.cmd`, que precisa de shell; e passar argumentos soltos a um shell gera o aviso DEP0190.
const build = spawnSync(`npx tauri build --bundles ${platform.bundle} --config src-tauri/tauri.sidecar.conf.json`, {
  cwd: root,
  env,
  stdio: "inherit",
  shell: true,
});
if (build.status !== 0) {
  restoreVersions();
  fail("o build falhou; a versão foi restaurada.");
}

function restoreVersions() {
  writeFileSync(paths.pkg, pkgText);
  writeFileSync(paths.cargo, cargoText);
}

// --- artefatos + latest.json ---
const metadata = JSON.parse(
  execFileSync("cargo", ["metadata", "--no-deps", "--format-version", "1", "--manifest-path", paths.cargo], { env, encoding: "utf8" }),
);
const installer = join(metadata.target_directory, "release", "bundle", platform.bundle, platform.file(next));
const signature = `${installer}.sig`;
if (!existsSync(installer) || !existsSync(signature)) fail(`não achei ${installer} (+ .sig).`);

mkdirSync(paths.out, { recursive: true });
const localInstaller = join(paths.out, basename(installer));
copyFileSync(installer, localInstaller);

const manifest = {
  version: next,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    [platform.key]: {
      signature: readFileSync(signature, "utf8").trim(),
      url: `${baseUrl}/${encodeURIComponent(basename(installer))}`,
    },
  },
};
const localManifest = join(paths.out, "latest.json");
writeFileSync(localManifest, JSON.stringify(manifest, null, 2));
console.log(`\n✔ ${basename(localInstaller)} e latest.json em instaladores/`);

if (dryRun) {
  // Ensaio não deixa rastro: a próxima release de verdade parte da mesma versão.
  restoreVersions();
  console.log(`Dry-run: nada foi enviado e a versão voltou para ${current}.`);
  console.log("Os arquivos acima são exatamente os que seriam publicados.");
  process.exit(0);
}

// --- upload: instalador primeiro, latest.json por último ---
// Assim, ninguém recebe o aviso de uma versão cujo instalador ainda não chegou à VPS.
const remote = (file) => `${target.ssh}:${target.remoteDir.replace(/\/$/, "")}/${basename(file)}`;
const portArgs = target.port ? ["-P", String(target.port)] : [];
for (const file of [localInstaller, localManifest]) {
  console.log(`Enviando ${basename(file)}…`);
  const upload = spawnSync("scp", [...portArgs, file, remote(file)], { stdio: "inherit" });
  if (upload.status !== 0) fail(`falha ao enviar ${basename(file)} (scp). A versão local já está em ${next}.`);
}
console.log(`\n✔ GlobalMusic ${next} publicado em ${baseUrl}/latest.json`);

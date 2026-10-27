// Baixa o yt-dlp oficial (versão "onedir": executável + pasta `_internal/`) para
// `src-tauri/binaries/yt-dlp/`, confere o SHA-256 e deixa o executável com nome fixo
// (`yt-dlp.exe` no Windows, `yt-dlp` no resto). O instalador copia a pasta inteira
// (`tauri.sidecar.conf.json` → `bundle.resources`).
//
// Por que onedir e não o executável único? O executável único se descompacta numa pasta
// temporária a cada execução: medimos 1,2–2,4 s só para iniciar, contra ~0,45 s do onedir.
// O app chama o yt-dlp uma vez por música, então isso é ~1 s a menos em cada faixa.
//
//   npm run setup:yt-dlp
//   npm run setup:yt-dlp -- universal-apple-darwin   (build universal do macOS no CI)

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { chmodSync, existsSync, mkdirSync, readdirSync, renameSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const RELEASE = "https://github.com/yt-dlp/yt-dlp/releases/latest/download";

// Target do Rust. Ordem:
// 1. argumento (ex.: o macOS universal, cujo yt-dlp_macos já serve Intel e ARM);
// 2. `rustc -vV`, a fonte oficial;
// 3. deduzido pelo Node, se o rustc não estiver no PATH deste terminal.
function rustcHost() {
  try {
    return execFileSync("rustc", ["-vV"], { encoding: "utf8" }).match(/^host: (\S+)$/m)?.[1];
  } catch {
    return undefined;
  }
}
const NODE_TO_RUST = {
  "win32-x64": "x86_64-pc-windows-msvc",
  "win32-arm64": "aarch64-pc-windows-msvc",
  "darwin-x64": "x86_64-apple-darwin",
  "darwin-arm64": "aarch64-apple-darwin",
  "linux-x64": "x86_64-unknown-linux-gnu",
  "linux-arm64": "aarch64-unknown-linux-gnu",
};
const triple = process.argv[2] || rustcHost() || NODE_TO_RUST[`${process.platform}-${process.arch}`];
if (!triple) throw new Error(`Sistema não reconhecido: ${process.platform}-${process.arch}. Passe o target como argumento.`);

const windows = triple.includes("windows");
const asset = windows
  ? triple.startsWith("aarch64")
    ? "yt-dlp_win_arm64.zip"
    : "yt-dlp_win.zip"
  : triple.includes("apple")
    ? "yt-dlp_macos.zip"
    : triple.startsWith("aarch64")
      ? "yt-dlp_linux_aarch64.zip"
      : "yt-dlp_linux.zip";

const binaries = join(dirname(fileURLToPath(import.meta.url)), "..", "src-tauri", "binaries");
const outDir = join(binaries, "yt-dlp");
const exeName = windows ? "yt-dlp.exe" : "yt-dlp";

async function download(url) {
  const res = await fetch(url, { redirect: "follow" });
  if (!res.ok) throw new Error(`${url} → HTTP ${res.status}`);
  return Buffer.from(await res.arrayBuffer());
}

/** `tar` do Windows e do macOS abre .zip; no Linux, `unzip` ou o módulo zipfile do Python. */
function unzip(zip, dest) {
  const attempts = [
    ["tar", ["-xf", zip, "-C", dest]],
    ["unzip", ["-q", "-o", zip, "-d", dest]],
    ["python3", ["-m", "zipfile", "-e", zip, dest]],
  ];
  for (const [cmd, args] of attempts) {
    try {
      execFileSync(cmd, args, { stdio: "ignore" });
      return;
    } catch {
      // tenta o próximo
    }
  }
  throw new Error("Não consegui extrair o .zip (precisa de tar, unzip ou python3).");
}

console.log(`Baixando ${asset} para ${triple}…`);
const [archive, sums] = await Promise.all([download(`${RELEASE}/${asset}`), download(`${RELEASE}/SHA2-256SUMS`)]);

const expected = sums
  .toString("utf8")
  .split("\n")
  .map((line) => line.trim().split(/\s+/))
  .find(([, name]) => name === asset)?.[0];
const actual = createHash("sha256").update(archive).digest("hex");
if (!expected || expected !== actual) {
  throw new Error(`SHA-256 não confere para ${asset} (esperado ${expected ?? "?"}, obtido ${actual}).`);
}

rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });
const zipPath = join(binaries, asset);
writeFileSync(zipPath, archive);
try {
  unzip(zipPath, outDir);
} finally {
  rmSync(zipPath, { force: true });
}

// O executável vem com o nome do pacote (yt-dlp.exe, yt-dlp_macos, yt-dlp_linux…): padroniza.
const exe = readdirSync(outDir).find((name) => name.startsWith("yt-dlp") && statSync(join(outDir, name)).isFile());
if (!exe || !existsSync(join(outDir, "_internal"))) {
  throw new Error(`O ${asset} não tem o formato esperado (executável + _internal/).`);
}
if (exe !== exeName) renameSync(join(outDir, exe), join(outDir, exeName));
if (!windows) chmodSync(join(outDir, exeName), 0o755);

// O executável único antigo (yt-dlp-<target>) não é mais usado.
for (const name of readdirSync(binaries)) {
  if (/^yt-dlp-.+/.test(name)) rmSync(join(binaries, name), { force: true });
}

console.log(`OK: ${join(outDir, exeName)} (${(archive.length / 1e6).toFixed(1)} MB, sha256 verificado)`);

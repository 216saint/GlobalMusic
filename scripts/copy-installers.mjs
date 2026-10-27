// Copia os instaladores gerados pelo `tauri build` para `instaladores/` na raiz
// do projeto. O build em si acontece fora do OneDrive (ver .cargo/config.toml),
// e é lá que o Tauri deixa os instaladores — difícil de achar.
//
// Roda via `node scripts/with-cargo.mjs` (precisa do cargo para achar a pasta de build).

import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const metadata = JSON.parse(
  execFileSync("cargo", ["metadata", "--no-deps", "--format-version", "1", "--manifest-path", join(root, "src-tauri", "Cargo.toml")], {
    encoding: "utf8",
  }),
);
const bundleDir = join(metadata.target_directory, "release", "bundle");
const outDir = join(root, "instaladores");
const INSTALLER = /\.(exe|msi|dmg|deb|rpm|AppImage)$/;

mkdirSync(outDir, { recursive: true });
let copied = 0;
for (const kind of existsSync(bundleDir) ? readdirSync(bundleDir) : []) {
  for (const file of readdirSync(join(bundleDir, kind)).filter((f) => INSTALLER.test(f))) {
    copyFileSync(join(bundleDir, kind, file), join(outDir, file));
    console.log(`instaladores/${file}`);
    copied++;
  }
}
if (!copied) console.log(`Nenhum instalador encontrado em ${bundleDir}.`);

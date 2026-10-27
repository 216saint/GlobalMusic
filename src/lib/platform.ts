import { isTauri } from "@tauri-apps/api/core";
import { platform } from "@tauri-apps/plugin-os";

export type Os = "windows" | "macos" | "linux" | "web";

export function currentOs(): Os {
  if (!isTauri()) return "web";
  const p = platform();
  return p === "windows" || p === "macos" ? p : "linux";
}

/** Windows e macOS têm vidro nativo (Acrylic/Vibrancy); no Linux a UI desenha o fundo. */
export const hasNativeGlass = (os: Os) => os === "windows" || os === "macos";

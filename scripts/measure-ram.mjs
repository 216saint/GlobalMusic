// Mede a RAM do GlobalMusic em execução: o processo do app + todos os
// processos filhos (no Windows, os msedgewebview2.exe da WebView2).
// A meta do projeto é < 150 MB somando tudo.
//
//   npm run measure:ram

import { execFileSync } from "node:child_process";

const BUDGET_MB = 150;
const APP_NAMES = ["globalmusic", "GlobalMusic"];
const mb = (bytes) => bytes / 1024 / 1024;

function windows() {
  // Win32_PerfFormattedData_* não é traduzido (o contador "Working Set - Private" é, no Windows pt-BR).
  const script = `
    $procs = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, Name
    $perf = @{}; Get-CimInstance Win32_PerfFormattedData_PerfProc_Process | ForEach-Object { $perf[[int]$_.IDProcess] = $_.WorkingSetPrivate }
    $procs | ForEach-Object { [pscustomobject]@{ pid = [int]$_.ProcessId; ppid = [int]$_.ParentProcessId; name = $_.Name; bytes = [double]($perf[[int]$_.ProcessId]) } } | ConvertTo-Json -Compress`;
  const out = execFileSync("powershell", ["-NoProfile", "-Command", script], { encoding: "utf8", maxBuffer: 64 << 20 });
  return JSON.parse(out);
}

function unix() {
  const out = execFileSync("ps", ["-axo", "pid=,ppid=,rss=,comm="], { encoding: "utf8" });
  return out
    .trim()
    .split("\n")
    .map((line) => {
      const [pid, ppid, rss, ...comm] = line.trim().split(/\s+/);
      return { pid: +pid, ppid: +ppid, name: comm.join(" ").split("/").pop(), bytes: +rss * 1024 };
    });
}

const procs = process.platform === "win32" ? windows() : unix();
const roots = procs.filter((p) => APP_NAMES.some((n) => p.name.replace(/\.exe$/i, "") === n));
if (!roots.length) {
  console.error("GlobalMusic não está rodando. Abra o app (de preferência o build de release) e rode de novo.");
  process.exit(1);
}

const tree = new Map(roots.map((p) => [p.pid, p]));
for (let grew = true; grew; ) {
  grew = false;
  for (const p of procs) {
    if (!tree.has(p.pid) && tree.has(p.ppid)) {
      tree.set(p.pid, p);
      grew = true;
    }
  }
}

const rows = [...tree.values()].sort((a, b) => b.bytes - a.bytes);
const total = rows.reduce((sum, p) => sum + p.bytes, 0);
for (const p of rows) console.log(`${mb(p.bytes).toFixed(1).padStart(8)} MB  ${p.name} (${p.pid})`);
console.log("-".repeat(40));
console.log(`${mb(total).toFixed(1).padStart(8)} MB  total (${rows.length} processos) — meta: < ${BUDGET_MB} MB`);
if (process.platform === "darwin") {
  console.log("Obs.: no macOS os processos do WebKit não são filhos do app; confira-os no Monitor de Atividade.");
}
process.exit(mb(total) < BUDGET_MB ? 0 : 2);

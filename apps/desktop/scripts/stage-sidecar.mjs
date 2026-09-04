import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const requestedProfile = process.argv[2] ?? "debug";
if (!new Set(["debug", "release"]).has(requestedProfile)) {
  throw new Error(`Unsupported sidecar profile: ${requestedProfile}`);
}
if (process.platform !== "win32") {
  throw new Error("Assisstant Desktop sidecar staging currently supports Windows hosts only.");
}

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const desktopDir = path.resolve(scriptDir, "..");
const repoRoot = path.resolve(desktopDir, "../..");
const tauriDir = path.join(desktopDir, "src-tauri");
const targetTriple = execFileSync("rustc", ["--print", "host-tuple"], {
  cwd: repoRoot,
  encoding: "utf8",
}).trim();

if (!targetTriple) throw new Error("rustc did not return a host target triple");

const cargoArgs = [
  "build",
  "-p",
  "windows-mcp",
  "--bin",
  "assistant-mcp",
  "--locked",
];
if (requestedProfile === "release") cargoArgs.push("--release");

execFileSync("cargo", cargoArgs, {
  cwd: repoRoot,
  stdio: "inherit",
});

const configuredTargetDir = process.env.CARGO_TARGET_DIR;
const targetDir = configuredTargetDir
  ? path.resolve(repoRoot, configuredTargetDir)
  : path.join(repoRoot, "target");
const source = path.join(targetDir, requestedProfile, "assistant-mcp.exe");
if (!existsSync(source)) {
  throw new Error(`Expected sidecar binary was not produced: ${source}`);
}

const binariesDir = path.join(tauriDir, "binaries");
mkdirSync(binariesDir, { recursive: true });
const destination = path.join(
  binariesDir,
  `assistant-mcp-${targetTriple}.exe`,
);
copyFileSync(source, destination);

console.log(`Staged assistant-mcp sidecar: ${destination}`);

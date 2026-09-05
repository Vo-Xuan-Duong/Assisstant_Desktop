import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync } from "node:fs";
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

const assistantArgs = [
  "build",
  "-p",
  "assisstant-desktop",
  "--bin",
  "assistant",
  "--locked",
];
if (requestedProfile === "release") assistantArgs.push("--release");
execFileSync("cargo", assistantArgs, {
  cwd: repoRoot,
  stdio: "inherit",
});

// Sherpa stays in DLLs to isolate its bundled protobuf from SentencePiece's.
const voiceArgs = ["build", "-p", "voice-runtime", "--features", "wake-sherpa", "--locked"];
if (requestedProfile === "release") voiceArgs.push("--release");
execFileSync("cargo", voiceArgs, { cwd: repoRoot, stdio: "inherit" });

const configuredTargetDir = process.env.CARGO_TARGET_DIR;
const targetDir = configuredTargetDir
  ? path.resolve(repoRoot, configuredTargetDir)
  : path.join(repoRoot, "target");
const source = path.join(targetDir, requestedProfile, "assistant-mcp.exe");
if (!existsSync(source)) {
  throw new Error(`Expected sidecar binary was not produced: ${source}`);
}
const assistantSource = path.join(targetDir, requestedProfile, "assistant.exe");
if (!existsSync(assistantSource)) {
  throw new Error(`Expected management CLI was not produced: ${assistantSource}`);
}

const binariesDir = path.join(tauriDir, "binaries");
mkdirSync(binariesDir, { recursive: true });
const destination = path.join(
  binariesDir,
  `assistant-mcp-${targetTriple}.exe`,
);
copyFileSync(source, destination);
const assistantDestination = path.join(
  binariesDir,
  `assistant-${targetTriple}.exe`,
);
copyFileSync(assistantSource, assistantDestination);

const runtimeDir = path.join(targetDir, requestedProfile);
const runtimeDlls = readdirSync(runtimeDir).filter((name) =>
  /^(sherpa-onnx|onnxruntime).*\.dll$/i.test(name),
);
if (!runtimeDlls.some((name) => name === "sherpa-onnx-c-api.dll")) {
  throw new Error("Sherpa runtime DLLs were not produced by the native build.");
}
for (const name of runtimeDlls) {
  copyFileSync(path.join(runtimeDir, name), path.join(binariesDir, name));
  // Test executables live in deps. Windows searches the executable directory
  // before System32, which can contain an incompatible onnxruntime.dll.
  mkdirSync(path.join(runtimeDir, "deps"), { recursive: true });
  copyFileSync(path.join(runtimeDir, name), path.join(runtimeDir, "deps", name));
}

console.log(`Staged assistant-mcp sidecar: ${destination}`);
console.log(`Staged assistant management CLI: ${assistantDestination}`);

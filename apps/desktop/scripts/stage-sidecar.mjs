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

// The desktop package uses tauri-build, which validates every configured
// externalBin during its build script. On a clean machine those files do not
// exist yet because this staging script is responsible for producing them.
// Disable externalBin only for these helper Cargo builds to break that
// bootstrap cycle. The real Tauri dev/build invocation still uses the normal
// configuration after this script has staged all binaries.
const desktopHelperEnv = {
  ...process.env,
  TAURI_CONFIG: JSON.stringify({ bundle: { externalBin: [] } }),
};
const desktopHelperArgs = [
  "build",
  "-p",
  "assisstant-desktop",
  "--bin",
  "assistant",
  "--bin",
  "assistant-root",
  "--bin",
  "assistant-satellite",
  "--bin",
  "assistant-satellite-remote",
  "--bin",
  "assistant-tts",
  "--locked",
];
if (requestedProfile === "release") desktopHelperArgs.push("--release");
execFileSync("cargo", desktopHelperArgs, {
  cwd: repoRoot,
  stdio: "inherit",
  env: desktopHelperEnv,
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
const assistantCoreSource = path.join(targetDir, requestedProfile, "assistant.exe");
if (!existsSync(assistantCoreSource)) {
  throw new Error(`Expected management CLI was not produced: ${assistantCoreSource}`);
}
const assistantRouterSource = path.join(targetDir, requestedProfile, "assistant-root.exe");
if (!existsSync(assistantRouterSource)) {
  throw new Error(`Expected canonical assistant router was not produced: ${assistantRouterSource}`);
}
const satelliteSource = path.join(targetDir, requestedProfile, "assistant-satellite.exe");
if (!existsSync(satelliteSource)) {
  throw new Error(`Expected satellite management helper was not produced: ${satelliteSource}`);
}
const satelliteRemoteSource = path.join(targetDir, requestedProfile, "assistant-satellite-remote.exe");
if (!existsSync(satelliteRemoteSource)) {
  throw new Error(`Expected remote satellite helper was not produced: ${satelliteRemoteSource}`);
}
const ttsSource = path.join(targetDir, requestedProfile, "assistant-tts.exe");
if (!existsSync(ttsSource)) {
  throw new Error(`Expected TTS management helper was not produced: ${ttsSource}`);
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
copyFileSync(assistantRouterSource, assistantDestination);
const assistantCoreDestination = path.join(
  binariesDir,
  `assistant-core-${targetTriple}.exe`,
);
copyFileSync(assistantCoreSource, assistantCoreDestination);
const satelliteDestination = path.join(
  binariesDir,
  `assistant-satellite-${targetTriple}.exe`,
);
copyFileSync(satelliteSource, satelliteDestination);
const satelliteRemoteDestination = path.join(
  binariesDir,
  `assistant-satellite-remote-${targetTriple}.exe`,
);
copyFileSync(satelliteRemoteSource, satelliteRemoteDestination);
const ttsDestination = path.join(
  binariesDir,
  `assistant-tts-${targetTriple}.exe`,
);
copyFileSync(ttsSource, ttsDestination);

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
console.log(`Staged canonical assistant CLI: ${assistantDestination}`);
console.log(`Staged assistant core management CLI: ${assistantCoreDestination}`);
console.log(`Staged satellite management helper: ${satelliteDestination}`);
console.log(`Staged remote satellite helper: ${satelliteRemoteDestination}`);
console.log(`Staged TTS management helper: ${ttsDestination}`);

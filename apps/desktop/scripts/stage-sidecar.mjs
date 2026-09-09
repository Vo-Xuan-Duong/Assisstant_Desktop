import { execFileSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
} from "node:fs";
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

// The desktop package uses tauri-build, which validates configured external
// binaries and resource globs during its build script. On a clean machine
// those files do not exist yet because this staging script is responsible for
// producing them. Disable both checks only for these helper Cargo builds to
// break that bootstrap cycle. The real Tauri dev/build invocation still uses
// the normal configuration after this script has staged every file.
const desktopHelperEnv = {
  ...process.env,
  TAURI_CONFIG: JSON.stringify({
    bundle: {
      externalBin: [],
      resources: [],
    },
  }),
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

function lockedPackageVersion(packageName) {
  const lockfile = readFileSync(path.join(repoRoot, "Cargo.lock"), "utf8");
  for (const block of lockfile.split(/\r?\n\[\[package\]\]\r?\n/)) {
    const name = block.match(/^name = "([^"]+)"$/m)?.[1];
    if (name !== packageName) continue;
    const version = block.match(/^version = "([^"]+)"$/m)?.[1];
    if (!version) {
      throw new Error(`Cargo.lock entry for ${packageName} has no version.`);
    }
    return version;
  }
  throw new Error(`Cargo.lock does not contain package ${packageName}.`);
}

function collectSherpaRuntimeDlls(directory) {
  if (!existsSync(directory)) return [];
  return readdirSync(directory)
    .filter((name) => /^(sherpa-onnx|onnxruntime).*\.dll$/i.test(name))
    .map((name) => path.join(directory, name));
}

function collectSherpaRuntimeDllsRecursive(directory, depth = 0) {
  if (!existsSync(directory) || depth > 5) return [];
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...collectSherpaRuntimeDllsRecursive(entryPath, depth + 1));
    } else if (entry.isFile() && /^(sherpa-onnx|onnxruntime).*\.dll$/i.test(entry.name)) {
      files.push(entryPath);
    }
  }
  return files;
}

function hasRequiredSherpaRuntime(files) {
  const names = new Set(files.map((file) => path.basename(file).toLowerCase()));
  return names.has("sherpa-onnx-c-api.dll") && names.has("onnxruntime.dll");
}

const runtimeDir = path.join(targetDir, requestedProfile);
let runtimeDllSources = collectSherpaRuntimeDlls(runtimeDir);
if (!hasRequiredSherpaRuntime(runtimeDllSources)) {
  const sherpaVersion = lockedPackageVersion("sherpa-onnx-sys");
  const archivePlatform =
    targetTriple === "x86_64-pc-windows-msvc"
      ? "win-x64"
      : targetTriple === "aarch64-pc-windows-msvc"
        ? "win-arm64"
        : null;
  if (!archivePlatform) {
    throw new Error(`Unsupported Windows target for Sherpa runtime staging: ${targetTriple}`);
  }

  // sherpa-onnx-sys owns this cache and may place runtime DLLs below different
  // subdirectories across crate releases. Stay scoped to the exact locked
  // version/platform cache instead of assuming a specific `lib`/`bin` layout.
  const lockedPrebuiltDir = path.join(
    targetDir,
    "sherpa-onnx-prebuilt",
    `sherpa-onnx-v${sherpaVersion}-${archivePlatform}-shared-MT-Release-lib`,
  );
  runtimeDllSources = collectSherpaRuntimeDllsRecursive(lockedPrebuiltDir);
}
if (!hasRequiredSherpaRuntime(runtimeDllSources)) {
  throw new Error(
    "Sherpa runtime DLLs were not produced by the native build or its locked prebuilt cache.",
  );
}

const uniqueRuntimeDlls = new Map();
for (const runtimeDllSource of runtimeDllSources) {
  const name = path.basename(runtimeDllSource);
  if (!uniqueRuntimeDlls.has(name.toLowerCase())) {
    uniqueRuntimeDlls.set(name.toLowerCase(), runtimeDllSource);
  }
}

const testDepsDir = path.join(runtimeDir, "deps");
mkdirSync(testDepsDir, { recursive: true });
for (const runtimeDllSource of uniqueRuntimeDlls.values()) {
  const name = path.basename(runtimeDllSource);
  copyFileSync(runtimeDllSource, path.join(binariesDir, name));
  // Test executables live in deps. Windows searches the executable directory
  // before System32, which can contain an incompatible onnxruntime.dll.
  copyFileSync(runtimeDllSource, path.join(testDepsDir, name));
}

console.log(`Staged assistant-mcp sidecar: ${destination}`);
console.log(`Staged canonical assistant CLI: ${assistantDestination}`);
console.log(`Staged assistant core management CLI: ${assistantCoreDestination}`);
console.log(`Staged satellite management helper: ${satelliteDestination}`);
console.log(`Staged remote satellite helper: ${satelliteRemoteDestination}`);
console.log(`Staged TTS management helper: ${ttsDestination}`);
console.log(
  `Staged Sherpa runtime DLLs: ${[...uniqueRuntimeDlls.values()].map((file) => path.basename(file)).join(", ")}`,
);

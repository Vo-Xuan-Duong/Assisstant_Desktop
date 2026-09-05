use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
};

use assistant_common::ToolRisk;
use permission_engine::{PermissionDecision, PermissionOverrideSnapshot};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;
use windows_tools::TOOL_CATALOG;

const APP_IDENTIFIER: &str = "com.voduong.assisstantdesktop";
const ZIPFORMER_DIR_NAME: &str = "sherpa-onnx-zipformer-vi-30M-int8-2026-02-09";
const WAKE_DIR_NAME: &str = "sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01";

type CliResult<T> = Result<T, String>;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> CliResult<()> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    let data_dir = extract_data_dir(&mut args)?;
    let paths = AppPaths::resolve(data_dir)?;

    if args.is_empty() {
        return run_tui(&paths);
    }

    match args[0].as_str() {
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        "version" | "--version" | "-V" => {
            println!("assistant {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "status" => command_status(&paths, args.iter().skip(1).any(|arg| arg == "--json")),
        "paths" => command_paths(&paths),
        "ai" => command_ai(&paths, &args[1..]),
        "wake" => command_wake(&paths, &args[1..]),
        "resources" => command_resources(&paths, &args[1..]),
        "permissions" => command_permissions(&paths, &args[1..]),
        "doctor" => command_doctor(&paths),
        other => Err(format!(
            "unknown command `{other}`. Run `assistant help` for available commands."
        )),
    }
}

fn print_help() {
    println!(
        r#"Assisstant Desktop terminal management

USAGE
  assistant [--data-dir <absolute-path>]
  assistant <command> [options]

COMMANDS
  status [--json]                         Runtime/config snapshot
  paths                                   Show shared local paths
  doctor                                  Local readiness diagnostics

  ai show                                 Show Antigravity configuration
  ai models                               Run `agy models`
  ai set --model <id>                     Persist model selection
  ai set --effort <value>                 Persist reasoning effort
  ai reset                                Reset AI settings to defaults

  wake show                               Show wake preferences
  wake enable                             Enable wake on next runtime load
  wake disable                            Disable wake on next runtime load
  wake phrase <text>                      Persist wake phrase label

  resources list                          Inspect STT/wake model files

  permissions list                        List native tool policy
  permissions set <tool> <allow|ask|deny> Set a Moderate-tool override
  permissions clear <tool>                Clear a Moderate-tool override

Running `assistant` without a command opens the interactive terminal dashboard.

ENVIRONMENT
  ASSISTANT_APP_DATA
  ASSISTANT_ZIPFORMER_MODEL_DIR
  ASSISTANT_WAKE_MODEL_DIR
  ASSISTANT_PERMISSION_POLICY_PATH
  ASSISTANT_ANTIGRAVITY_BIN
"#
    );
}

fn extract_data_dir(args: &mut Vec<String>) -> CliResult<Option<PathBuf>> {
    let mut result = None;
    let mut index = 0usize;
    while index < args.len() {
        if args[index] == "--data-dir" {
            if index + 1 >= args.len() {
                return Err("--data-dir requires an absolute path".into());
            }
            result = Some(PathBuf::from(args.remove(index + 1)));
            args.remove(index);
            continue;
        }
        if let Some(value) = args[index].strip_prefix("--data-dir=") {
            result = Some(PathBuf::from(value));
            args.remove(index);
            continue;
        }
        index += 1;
    }

    Ok(result.or_else(|| env::var_os("ASSISTANT_APP_DATA").map(PathBuf::from)))
}

#[derive(Debug, Clone)]
struct AppPaths {
    root: PathBuf,
    antigravity_settings: PathBuf,
    wake_settings: PathBuf,
    permission_policy: PathBuf,
    stt_model_dir: PathBuf,
    wake_model_dir: PathBuf,
}

impl AppPaths {
    fn resolve(override_root: Option<PathBuf>) -> CliResult<Self> {
        let root = match override_root {
            Some(path) => require_absolute("application data path", path)?,
            None => default_app_data_dir()?,
        };
        let stt_model_dir = absolute_env_override(
            "ASSISTANT_ZIPFORMER_MODEL_DIR",
            root.join("models").join("stt").join(ZIPFORMER_DIR_NAME),
        )?;
        let wake_model_dir = absolute_env_override(
            "ASSISTANT_WAKE_MODEL_DIR",
            root.join("models").join("wake").join(WAKE_DIR_NAME),
        )?;
        let permission_policy = absolute_env_override(
            "ASSISTANT_PERMISSION_POLICY_PATH",
            root.join("permissions").join("policy.json"),
        )?;

        Ok(Self {
            antigravity_settings: root.join("settings").join("antigravity.json"),
            wake_settings: root.join("settings").join("wake.json"),
            permission_policy,
            root,
            stt_model_dir,
            wake_model_dir,
        })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AiSettings {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    effort: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct WakePreferences {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    phrase: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ResourceSummary {
    id: &'static str,
    label: &'static str,
    root: String,
    present: usize,
    required: usize,
    ready: bool,
    files: Vec<ResourceFileSummary>,
}

#[derive(Debug, Clone, Serialize)]
struct ResourceFileSummary {
    name: &'static str,
    exists: bool,
}

#[derive(Debug, Clone, Serialize)]
struct StatusSnapshot {
    app_data: String,
    runtime_running: Option<bool>,
    antigravity_binary: String,
    antigravity_available: bool,
    ai_model: Option<String>,
    ai_effort: Option<String>,
    wake_enabled: bool,
    wake_phrase: Option<String>,
    stt: ResourceSummary,
    wake: ResourceSummary,
}

impl StatusSnapshot {
    fn load(paths: &AppPaths) -> CliResult<Self> {
        let ai = load_json_or_default::<AiSettings>(&paths.antigravity_settings)?;
        let wake_preferences = load_json_or_default::<WakePreferences>(&paths.wake_settings)?;
        let antigravity_binary = resolve_antigravity_binary();
        let antigravity_available = command_available(&antigravity_binary);

        Ok(Self {
            app_data: paths.root.display().to_string(),
            runtime_running: runtime_running(),
            antigravity_binary,
            antigravity_available,
            ai_model: ai.model,
            ai_effort: ai.effort,
            wake_enabled: wake_preferences.enabled,
            wake_phrase: wake_preferences.phrase,
            stt: stt_resource(paths),
            wake: wake_resource(paths),
        })
    }
}

fn command_status(paths: &AppPaths, json: bool) -> CliResult<()> {
    let snapshot = StatusSnapshot::load(paths)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&snapshot)
                .map_err(|error| format!("cannot serialize status: {error}"))?
        );
        return Ok(());
    }

    println!("Assisstant Desktop");
    println!("  Runtime        {}", runtime_name(snapshot.runtime_running));
    println!(
        "  Antigravity    {} ({})",
        ready_name(snapshot.antigravity_available),
        snapshot.antigravity_binary
    );
    println!(
        "  AI model       {}",
        snapshot.ai_model.as_deref().unwrap_or("default")
    );
    println!(
        "  AI effort      {}",
        snapshot.ai_effort.as_deref().unwrap_or("default")
    );
    println!(
        "  Wake           {}",
        if snapshot.wake_enabled {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!(
        "  STT            {}/{} {}",
        snapshot.stt.present,
        snapshot.stt.required,
        ready_name(snapshot.stt.ready)
    );
    println!(
        "  Wake model     {}/{} {}",
        snapshot.wake.present,
        snapshot.wake.required,
        ready_name(snapshot.wake.ready)
    );
    println!("  Data           {}", snapshot.app_data);
    Ok(())
}

fn command_paths(paths: &AppPaths) -> CliResult<()> {
    println!("app_data          {}", paths.root.display());
    println!("ai_settings       {}", paths.antigravity_settings.display());
    println!("wake_settings     {}", paths.wake_settings.display());
    println!("permission_policy {}", paths.permission_policy.display());
    println!("stt_model         {}", paths.stt_model_dir.display());
    println!("wake_model        {}", paths.wake_model_dir.display());
    Ok(())
}

fn command_ai(paths: &AppPaths, args: &[String]) -> CliResult<()> {
    let subcommand = args.first().map(String::as_str).unwrap_or("show");
    match subcommand {
        "show" => {
            let settings = load_json_or_default::<AiSettings>(&paths.antigravity_settings)?;
            let binary = resolve_antigravity_binary();
            println!("binary  {binary}");
            println!("status  {}", ready_name(command_available(&binary)));
            println!("model   {}", settings.model.as_deref().unwrap_or("default"));
            println!("effort  {}", settings.effort.as_deref().unwrap_or("default"));
        }
        "models" => {
            let binary = resolve_antigravity_binary();
            let output = Command::new(&binary)
                .arg("models")
                .output()
                .map_err(|error| format!("cannot run `{binary} models`: {error}"))?;
            if !output.status.success() {
                return Err(format!(
                    "`{binary} models` failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            print!("{}", String::from_utf8_lossy(&output.stdout));
        }
        "set" => {
            let mut model = None;
            let mut effort = None;
            let mut index = 1usize;
            while index < args.len() {
                match args[index].as_str() {
                    "--model" => {
                        model = Some(argument_value(args, index, "--model")?);
                        index += 2;
                    }
                    "--effort" => {
                        effort = Some(argument_value(args, index, "--effort")?);
                        index += 2;
                    }
                    other => return Err(format!("unknown ai set option `{other}`")),
                }
            }
            if model.is_none() && effort.is_none() {
                return Err("ai set requires --model and/or --effort".into());
            }

            let mut settings = load_json_or_default::<AiSettings>(&paths.antigravity_settings)?;
            if let Some(value) = model {
                settings.model = clean_optional(value);
            }
            if let Some(value) = effort {
                settings.effort = clean_optional(value);
            }
            save_json_atomic(&paths.antigravity_settings, &settings)?;
            println!("AI settings saved: {}", paths.antigravity_settings.display());
            print_reload_note();
        }
        "reset" => {
            save_json_atomic(&paths.antigravity_settings, &AiSettings::default())?;
            println!("AI settings reset to runtime defaults.");
            print_reload_note();
        }
        other => return Err(format!("unknown ai command `{other}`")),
    }
    Ok(())
}

fn command_wake(paths: &AppPaths, args: &[String]) -> CliResult<()> {
    let subcommand = args.first().map(String::as_str).unwrap_or("show");
    let mut preferences = load_json_or_default::<WakePreferences>(&paths.wake_settings)?;

    match subcommand {
        "show" => {
            println!("enabled  {}", preferences.enabled);
            println!("phrase   {}", preferences.phrase.as_deref().unwrap_or("default"));
            println!("file     {}", paths.wake_settings.display());
            return Ok(());
        }
        "enable" => preferences.enabled = true,
        "disable" => preferences.enabled = false,
        "phrase" => {
            if args.len() < 2 {
                return Err("wake phrase requires text".into());
            }
            let phrase = args[1..].join(" ");
            let phrase = phrase.trim();
            if phrase.is_empty() {
                return Err("wake phrase cannot be empty".into());
            }
            preferences.phrase = Some(phrase.to_owned());
        }
        other => return Err(format!("unknown wake command `{other}`")),
    }

    save_json_atomic(&paths.wake_settings, &preferences)?;
    println!("Wake settings saved: {}", paths.wake_settings.display());
    print_reload_note();
    Ok(())
}

fn command_resources(paths: &AppPaths, args: &[String]) -> CliResult<()> {
    let subcommand = args.first().map(String::as_str).unwrap_or("list");
    if subcommand != "list" {
        return Err(format!("unknown resources command `{subcommand}`"));
    }

    for resource in [stt_resource(paths), wake_resource(paths)] {
        println!(
            "{}  {}/{}  {}",
            resource.id,
            resource.present,
            resource.required,
            ready_name(resource.ready)
        );
        println!("  {}", resource.root);
        for file in resource.files {
            println!("  [{}] {}", if file.exists { "x" } else { " " }, file.name);
        }
    }
    Ok(())
}

fn command_permissions(paths: &AppPaths, args: &[String]) -> CliResult<()> {
    let subcommand = args.first().map(String::as_str).unwrap_or("list");
    match subcommand {
        "list" => {
            let policy = load_json_or_default::<PermissionOverrideSnapshot>(&paths.permission_policy)?;
            print_permission_policy(&policy);
        }
        "set" => {
            if args.len() != 3 {
                return Err("usage: assistant permissions set <tool> <allow|ask|deny>".into());
            }
            let tool_name = &args[1];
            let decision = parse_decision(&args[2])?;
            let definition = TOOL_CATALOG
                .iter()
                .find(|item| item.name == tool_name)
                .ok_or_else(|| format!("unknown tool `{tool_name}`"))?;
            if !matches!(definition.risk, ToolRisk::Moderate) {
                return Err(format!(
                    "only Moderate tools are user-overridable; `{}` is {}",
                    definition.name,
                    risk_name(definition.risk)
                ));
            }

            let mut policy = load_json_or_default::<PermissionOverrideSnapshot>(&paths.permission_policy)?;
            policy.set(definition.name, decision);
            save_json_atomic(&paths.permission_policy, &policy)?;
            println!(
                "{} -> {} (revision {})",
                definition.name,
                decision_name(decision),
                policy.revision
            );
            print_reload_note();
        }
        "clear" => {
            if args.len() != 2 {
                return Err("usage: assistant permissions clear <tool>".into());
            }
            let tool_name = &args[1];
            let definition = TOOL_CATALOG
                .iter()
                .find(|item| item.name == tool_name)
                .ok_or_else(|| format!("unknown tool `{tool_name}`"))?;
            if !matches!(definition.risk, ToolRisk::Moderate) {
                return Err(format!("`{}` does not support a runtime override", definition.name));
            }

            let mut policy = load_json_or_default::<PermissionOverrideSnapshot>(&paths.permission_policy)?;
            policy.clear(definition.name);
            save_json_atomic(&paths.permission_policy, &policy)?;
            println!("{} override cleared (revision {})", definition.name, policy.revision);
            print_reload_note();
        }
        other => return Err(format!("unknown permissions command `{other}`")),
    }
    Ok(())
}

fn command_doctor(paths: &AppPaths) -> CliResult<()> {
    let mut failures = 0usize;
    println!("Assisstant Desktop doctor\n");

    doctor_line(
        "app data path",
        paths.root.is_absolute(),
        &paths.root.display().to_string(),
        &mut failures,
    );

    let binary = resolve_antigravity_binary();
    doctor_line(
        "Antigravity CLI",
        command_available(&binary),
        &binary,
        &mut failures,
    );

    match load_json_or_default::<AiSettings>(&paths.antigravity_settings) {
        Ok(_) => doctor_line("AI settings", true, "valid JSON/default", &mut failures),
        Err(error) => doctor_line("AI settings", false, &error, &mut failures),
    }
    match load_json_or_default::<WakePreferences>(&paths.wake_settings) {
        Ok(_) => doctor_line("wake settings", true, "valid JSON/default", &mut failures),
        Err(error) => doctor_line("wake settings", false, &error, &mut failures),
    }
    match load_json_or_default::<PermissionOverrideSnapshot>(&paths.permission_policy) {
        Ok(_) => doctor_line("permission policy", true, "valid JSON/default", &mut failures),
        Err(error) => doctor_line("permission policy", false, &error, &mut failures),
    }

    let stt = stt_resource(paths);
    doctor_line(
        "Vietnamese STT",
        stt.ready,
        &format!("{}/{} files at {}", stt.present, stt.required, stt.root),
        &mut failures,
    );

    let wake = wake_resource(paths);
    let wake_preferences = load_json_or_default::<WakePreferences>(&paths.wake_settings)
        .unwrap_or_default();
    doctor_line(
        "wake resources",
        !wake_preferences.enabled || wake.ready,
        &format!("{}/{} files at {}", wake.present, wake.required, wake.root),
        &mut failures,
    );

    println!("\nRuntime process: {}", runtime_name(runtime_running()));
    if failures == 0 {
        println!("Result: no local configuration/resource blockers detected.");
        Ok(())
    } else {
        Err(format!("doctor detected {failures} blocking issue(s)"))
    }
}

fn argument_value(args: &[String], index: usize, name: &str) -> CliResult<String> {
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("{name} requires a value"))
}

fn parse_decision(value: &str) -> CliResult<PermissionDecision> {
    match value.to_ascii_lowercase().as_str() {
        "allow" => Ok(PermissionDecision::Allow),
        "ask" => Ok(PermissionDecision::Ask),
        "deny" => Ok(PermissionDecision::Deny),
        _ => Err(format!("invalid decision `{value}`; expected allow, ask, or deny")),
    }
}

fn effective_decision(
    tool_name: &str,
    risk: ToolRisk,
    policy: &PermissionOverrideSnapshot,
) -> PermissionDecision {
    match risk {
        ToolRisk::Safe => PermissionDecision::Allow,
        ToolRisk::Moderate => policy
            .decision_for(tool_name)
            .unwrap_or(PermissionDecision::Allow),
        ToolRisk::Sensitive => PermissionDecision::Ask,
        ToolRisk::Blocked => PermissionDecision::Deny,
    }
}

fn print_permission_policy(policy: &PermissionOverrideSnapshot) {
    println!("REVISION {}", policy.revision);
    println!("RISK       DECISION   TOOL");
    for tool in TOOL_CATALOG {
        let decision = effective_decision(tool.name, tool.risk, policy);
        let override_mark = if matches!(tool.risk, ToolRisk::Moderate)
            && policy.decision_for(tool.name).is_some()
        {
            " *"
        } else {
            ""
        };
        println!(
            "{:<10} {:<10} {}{}",
            risk_name(tool.risk),
            decision_name(decision),
            tool.name,
            override_mark
        );
    }
    println!("\n* persisted Moderate-tool override");
}

fn stt_resource(paths: &AppPaths) -> ResourceSummary {
    resource_summary(
        "stt_zipformer_vi",
        "Vietnamese Zipformer STT",
        &paths.stt_model_dir,
        &[
            "encoder.int8.onnx",
            "decoder.onnx",
            "joiner.int8.onnx",
            "tokens.txt",
        ],
    )
}

fn wake_resource(paths: &AppPaths) -> ResourceSummary {
    resource_summary(
        "wake_word",
        "Wake Word Resources",
        &paths.wake_model_dir,
        &[
            "encoder-epoch-12-avg-2-chunk-16-left-64.int8.onnx",
            "decoder-epoch-12-avg-2-chunk-16-left-64.onnx",
            "joiner-epoch-12-avg-2-chunk-16-left-64.int8.onnx",
            "tokens.txt",
            "keywords.txt",
        ],
    )
}

fn resource_summary(
    id: &'static str,
    label: &'static str,
    root: &Path,
    names: &[&'static str],
) -> ResourceSummary {
    let files = names
        .iter()
        .map(|name| ResourceFileSummary {
            name,
            exists: root.join(name).is_file(),
        })
        .collect::<Vec<_>>();
    let present = files.iter().filter(|file| file.exists).count();
    ResourceSummary {
        id,
        label,
        root: root.display().to_string(),
        present,
        required: files.len(),
        ready: present == files.len(),
        files,
    }
}

fn load_json_or_default<T>(path: &Path) -> CliResult<T>
where
    T: DeserializeOwned + Default,
{
    if !path.is_file() {
        return Ok(T::default());
    }
    let bytes = fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))
}

fn save_json_atomic<T: Serialize>(path: &Path, value: &T) -> CliResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;

    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("cannot serialize {}: {error}", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("settings.json");
    let temporary = parent.join(format!(".{file_name}.{}.part", Uuid::new_v4()));
    let backup = parent.join(format!(".{file_name}.{}.bak", Uuid::new_v4()));

    fs::write(&temporary, bytes)
        .map_err(|error| format!("cannot write {}: {error}", temporary.display()))?;

    let had_existing = path.exists();
    if had_existing {
        if let Err(error) = fs::rename(path, &backup) {
            let _ = fs::remove_file(&temporary);
            return Err(format!("cannot stage previous {}: {error}", path.display()));
        }
    }

    if let Err(error) = fs::rename(&temporary, path) {
        if had_existing {
            let _ = fs::rename(&backup, path);
        }
        let _ = fs::remove_file(&temporary);
        return Err(format!("cannot commit {}: {error}", path.display()));
    }

    if had_existing {
        let _ = fs::remove_file(&backup);
    }
    Ok(())
}

fn default_app_data_dir() -> CliResult<PathBuf> {
    #[cfg(windows)]
    {
        let local = env::var_os("LOCALAPPDATA")
            .ok_or_else(|| "LOCALAPPDATA is not available".to_owned())?;
        return Ok(PathBuf::from(local).join(APP_IDENTIFIER));
    }

    #[cfg(not(windows))]
    {
        let home = env::var_os("HOME").ok_or_else(|| "HOME is not available".to_owned())?;
        Ok(PathBuf::from(home)
            .join(".local")
            .join("share")
            .join(APP_IDENTIFIER))
    }
}

fn absolute_env_override(name: &str, fallback: PathBuf) -> CliResult<PathBuf> {
    match env::var_os(name) {
        Some(value) => require_absolute(name, PathBuf::from(value)),
        None => Ok(fallback),
    }
}

fn require_absolute(name: &str, path: PathBuf) -> CliResult<PathBuf> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(format!("{name} must be an absolute path"))
    }
}

fn resolve_antigravity_binary() -> String {
    if let Ok(binary) = env::var("ASSISTANT_ANTIGRAVITY_BIN") {
        if !binary.trim().is_empty() {
            return binary;
        }
    }

    #[cfg(windows)]
    {
        if let Some(local_data) = env::var_os("LOCALAPPDATA") {
            let candidate = PathBuf::from(local_data)
                .join("agy")
                .join("bin")
                .join("agy.exe");
            if candidate.is_file() {
                return candidate.to_string_lossy().into_owned();
            }
        }
        if let Some(profile) = env::var_os("USERPROFILE") {
            let candidate = PathBuf::from(profile)
                .join(".gemini")
                .join("bin")
                .join("agy.exe");
            if candidate.is_file() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }

    "agy".to_owned()
}

fn command_available(binary: &str) -> bool {
    Command::new(binary)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

#[cfg(windows)]
fn runtime_running() -> Option<bool> {
    windows_tools::apps::list_running().ok().map(|apps| {
        apps.into_iter().any(|app| {
            app.executable.eq_ignore_ascii_case("assisstant-desktop.exe")
                || app.executable.eq_ignore_ascii_case("Assisstant Desktop.exe")
        })
    })
}

#[cfg(not(windows))]
fn runtime_running() -> Option<bool> {
    None
}

fn clean_optional(value: String) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("default") {
        None
    } else {
        Some(value.to_owned())
    }
}

fn print_reload_note() {
    println!(
        "Note: the durable shared config is updated. A currently running desktop process must be restarted before the change is guaranteed to be active; live management IPC is the next migration phase."
    );
}

fn doctor_line(label: &str, ok: bool, detail: &str, failures: &mut usize) {
    if !ok {
        *failures += 1;
    }
    println!("[{}] {:<20} {}", if ok { "OK" } else { "!!" }, label, detail);
}

fn ready_name(value: bool) -> &'static str {
    if value { "ready" } else { "not-ready" }
}

fn runtime_name(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "running",
        Some(false) => "stopped",
        None => "unknown",
    }
}

fn risk_name(risk: ToolRisk) -> &'static str {
    match risk {
        ToolRisk::Safe => "safe",
        ToolRisk::Moderate => "moderate",
        ToolRisk::Sensitive => "sensitive",
        ToolRisk::Blocked => "blocked",
    }
}

fn decision_name(decision: PermissionDecision) -> &'static str {
    match decision {
        PermissionDecision::Allow => "allow",
        PermissionDecision::Ask => "ask",
        PermissionDecision::Deny => "deny",
    }
}

#[derive(Debug, Clone, Copy)]
enum TuiPage {
    Dashboard,
    Resources,
    Ai,
    Permissions,
}

fn run_tui(paths: &AppPaths) -> CliResult<()> {
    let mut page = TuiPage::Dashboard;
    loop {
        let status = StatusSnapshot::load(paths)?;
        let policy = load_json_or_default::<PermissionOverrideSnapshot>(&paths.permission_policy)?;
        clear_terminal();
        render_tui_header(page);
        match page {
            TuiPage::Dashboard => render_tui_dashboard(&status),
            TuiPage::Resources => render_tui_resources(&status),
            TuiPage::Ai => render_tui_ai(&status),
            TuiPage::Permissions => print_permission_policy(&policy),
        }
        println!("\n[1] Dashboard  [2] Resources  [3] AI  [4] Permissions  [r] Refresh  [q] Quit");
        print!("assistant> ");
        io::stdout()
            .flush()
            .map_err(|error| format!("cannot flush terminal: {error}"))?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|error| format!("cannot read terminal input: {error}"))?;
        match input.trim().to_ascii_lowercase().as_str() {
            "1" => page = TuiPage::Dashboard,
            "2" => page = TuiPage::Resources,
            "3" => page = TuiPage::Ai,
            "4" => page = TuiPage::Permissions,
            "r" | "" => {}
            "q" | "quit" | "exit" => break,
            _ => {}
        }
    }
    clear_terminal();
    Ok(())
}

fn clear_terminal() {
    print!("\x1b[2J\x1b[H");
    let _ = io::stdout().flush();
}

fn render_tui_header(page: TuiPage) {
    let page = match page {
        TuiPage::Dashboard => "Dashboard",
        TuiPage::Resources => "Resources",
        TuiPage::Ai => "AI / Antigravity",
        TuiPage::Permissions => "Permissions",
    };
    println!("+------------------------------------------------------------------+");
    println!("| Assisstant Desktop CLI  {:<42}|", page);
    println!("+------------------------------------------------------------------+\n");
}

fn render_tui_dashboard(status: &StatusSnapshot) {
    println!("SYSTEM STATUS\n");
    println!("  Runtime       {}", runtime_name(status.runtime_running));
    println!("  Antigravity   {}", ready_name(status.antigravity_available));
    println!(
        "  STT           {}/{} {}",
        status.stt.present,
        status.stt.required,
        ready_name(status.stt.ready)
    );
    println!(
        "  Wake          {}",
        if status.wake_enabled { "enabled" } else { "disabled" }
    );
    println!(
        "  Wake model    {}/{} {}",
        status.wake.present,
        status.wake.required,
        ready_name(status.wake.ready)
    );
    println!(
        "  AI model      {}",
        status.ai_model.as_deref().unwrap_or("default")
    );
    println!(
        "  AI effort     {}",
        status.ai_effort.as_deref().unwrap_or("default")
    );
    println!("\nDATA\n  {}", status.app_data);
}

fn render_tui_resources(status: &StatusSnapshot) {
    for resource in [&status.stt, &status.wake] {
        println!(
            "{}  {}/{}  {}",
            resource.label,
            resource.present,
            resource.required,
            ready_name(resource.ready)
        );
        println!("  {}", resource.root);
        for file in &resource.files {
            println!("  [{}] {}", if file.exists { "x" } else { " " }, file.name);
        }
        println!();
    }
}

fn render_tui_ai(status: &StatusSnapshot) {
    println!("ANTIGRAVITY\n");
    println!("  CLI        {}", status.antigravity_binary);
    println!("  Status     {}", ready_name(status.antigravity_available));
    println!(
        "  Model      {}",
        status.ai_model.as_deref().unwrap_or("default")
    );
    println!(
        "  Effort     {}",
        status.ai_effort.as_deref().unwrap_or("default")
    );
    println!("\nCommands:");
    println!("  assistant ai models");
    println!("  assistant ai set --model <id> --effort <value>");
    println!("  assistant ai reset");
}

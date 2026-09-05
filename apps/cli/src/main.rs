use std::{
    env, fs,
    io::{self, Stdout},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use assistant_common::ToolRisk;
use clap::{Parser, Subcommand, ValueEnum};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use permission_engine::{PermissionDecision, PermissionOverrideSnapshot};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;
use windows_tools::TOOL_CATALOG;

const APP_IDENTIFIER: &str = "com.voduong.assisstantdesktop";
const ZIPFORMER_DIR_NAME: &str = "sherpa-onnx-zipformer-vi-30M-int8-2026-02-09";
const WAKE_DIR_NAME: &str = "sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01";

#[derive(Debug, Parser)]
#[command(
    name = "assistant",
    version,
    about = "Terminal management surface for Assisstant Desktop",
    long_about = "Manage the background Assisstant Desktop runtime configuration from the terminal. Running without a subcommand opens the TUI dashboard."
)]
struct Cli {
    /// Override the application data directory. This must be absolute.
    #[arg(long, global = true, env = "ASSISTANT_APP_DATA")]
    data_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Print a compact runtime/configuration snapshot.
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Show the exact shared paths used by CLI management.
    Paths,
    /// Manage Antigravity/Gemini settings.
    Ai {
        #[command(subcommand)]
        command: AiCommand,
    },
    /// Manage wake-word preferences.
    Wake {
        #[command(subcommand)]
        command: WakeCommand,
    },
    /// Inspect local native resources.
    Resources {
        #[command(subcommand)]
        command: ResourceCommand,
    },
    /// Inspect or update Moderate-tool policy overrides.
    Permissions {
        #[command(subcommand)]
        command: PermissionCommand,
    },
    /// Run local environment/readiness diagnostics.
    Doctor,
}

#[derive(Debug, Subcommand)]
enum AiCommand {
    Show,
    Models,
    Set {
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        effort: Option<String>,
    },
    Reset,
}

#[derive(Debug, Subcommand)]
enum WakeCommand {
    Show,
    Enable,
    Disable,
    Phrase { phrase: String },
}

#[derive(Debug, Subcommand)]
enum ResourceCommand {
    List,
}

#[derive(Debug, Subcommand)]
enum PermissionCommand {
    List,
    Set {
        tool: String,
        decision: DecisionArg,
    },
    Clear { tool: String },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DecisionArg {
    Allow,
    Ask,
    Deny,
}

impl From<DecisionArg> for PermissionDecision {
    fn from(value: DecisionArg) -> Self {
        match value {
            DecisionArg::Allow => Self::Allow,
            DecisionArg::Ask => Self::Ask,
            DecisionArg::Deny => Self::Deny,
        }
    }
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
    fn resolve(override_root: Option<PathBuf>) -> Result<Self> {
        let root = match override_root {
            Some(path) => require_absolute("--data-dir", path)?,
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
        Ok(Self {
            antigravity_settings: root.join("settings").join("antigravity.json"),
            wake_settings: root.join("settings").join("wake.json"),
            permission_policy: absolute_env_override(
                "ASSISTANT_PERMISSION_POLICY_PATH",
                root.join("permissions").join("policy.json"),
            )?,
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
    fn load(paths: &AppPaths) -> Result<Self> {
        let ai = load_json_or_default::<AiSettings>(&paths.antigravity_settings)?;
        let wake_preferences = load_json_or_default::<WakePreferences>(&paths.wake_settings)?;
        let binary = resolve_antigravity_binary();
        Ok(Self {
            app_data: paths.root.display().to_string(),
            runtime_running: runtime_running(),
            antigravity_available: command_available(&binary),
            antigravity_binary: binary,
            ai_model: ai.model,
            ai_effort: ai.effort,
            wake_enabled: wake_preferences.enabled,
            wake_phrase: wake_preferences.phrase,
            stt: stt_resource(paths),
            wake: wake_resource(paths),
        })
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = AppPaths::resolve(cli.data_dir)?;

    match cli.command {
        None => run_tui(paths),
        Some(Commands::Status { json }) => command_status(&paths, json),
        Some(Commands::Paths) => command_paths(&paths),
        Some(Commands::Ai { command }) => command_ai(&paths, command),
        Some(Commands::Wake { command }) => command_wake(&paths, command),
        Some(Commands::Resources { command }) => command_resources(&paths, command),
        Some(Commands::Permissions { command }) => command_permissions(&paths, command),
        Some(Commands::Doctor) => command_doctor(&paths),
    }
}

fn command_status(paths: &AppPaths, json: bool) -> Result<()> {
    let snapshot = StatusSnapshot::load(paths)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
        return Ok(());
    }

    println!("Assisstant Desktop");
    println!("  Runtime        {}", runtime_name(snapshot.runtime_running));
    println!(
        "  Antigravity    {} ({})",
        yes_no(snapshot.antigravity_available),
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
    println!("  Wake           {}", if snapshot.wake_enabled { "enabled" } else { "disabled" });
    println!(
        "  STT            {}/{} {}",
        snapshot.stt.present,
        snapshot.stt.required,
        if snapshot.stt.ready { "ready" } else { "incomplete" }
    );
    println!(
        "  Wake model     {}/{} {}",
        snapshot.wake.present,
        snapshot.wake.required,
        if snapshot.wake.ready { "ready" } else { "incomplete" }
    );
    println!("  Data           {}", snapshot.app_data);
    Ok(())
}

fn command_paths(paths: &AppPaths) -> Result<()> {
    println!("app_data          {}", paths.root.display());
    println!("ai_settings       {}", paths.antigravity_settings.display());
    println!("wake_settings     {}", paths.wake_settings.display());
    println!("permission_policy {}", paths.permission_policy.display());
    println!("stt_model         {}", paths.stt_model_dir.display());
    println!("wake_model        {}", paths.wake_model_dir.display());
    Ok(())
}

fn command_ai(paths: &AppPaths, command: AiCommand) -> Result<()> {
    match command {
        AiCommand::Show => {
            let settings = load_json_or_default::<AiSettings>(&paths.antigravity_settings)?;
            let binary = resolve_antigravity_binary();
            println!("binary  {binary}");
            println!("status  {}", yes_no(command_available(&binary)));
            println!("model   {}", settings.model.as_deref().unwrap_or("default"));
            println!("effort  {}", settings.effort.as_deref().unwrap_or("default"));
        }
        AiCommand::Models => {
            let binary = resolve_antigravity_binary();
            let output = Command::new(&binary)
                .arg("models")
                .output()
                .with_context(|| format!("cannot run `{binary} models`"))?;
            if !output.status.success() {
                bail!(
                    "`{binary} models` failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                );
            }
            print!("{}", String::from_utf8_lossy(&output.stdout));
        }
        AiCommand::Set { model, effort } => {
            if model.as_deref().is_none_or(str::is_empty)
                && effort.as_deref().is_none_or(str::is_empty)
            {
                bail!("provide --model and/or --effort");
            }
            let mut settings = load_json_or_default::<AiSettings>(&paths.antigravity_settings)?;
            if let Some(model) = model {
                settings.model = clean_optional(model);
            }
            if let Some(effort) = effort {
                settings.effort = clean_optional(effort);
            }
            save_json_atomic(&paths.antigravity_settings, &settings)?;
            println!("AI settings saved: {}", paths.antigravity_settings.display());
            print_restart_note();
        }
        AiCommand::Reset => {
            save_json_atomic(&paths.antigravity_settings, &AiSettings::default())?;
            println!("AI settings reset to runtime defaults.");
            print_restart_note();
        }
    }
    Ok(())
}

fn command_wake(paths: &AppPaths, command: WakeCommand) -> Result<()> {
    let mut preferences = load_json_or_default::<WakePreferences>(&paths.wake_settings)?;
    match command {
        WakeCommand::Show => {
            println!("enabled  {}", preferences.enabled);
            println!("phrase   {}", preferences.phrase.as_deref().unwrap_or("default"));
            println!("file     {}", paths.wake_settings.display());
            return Ok(());
        }
        WakeCommand::Enable => preferences.enabled = true,
        WakeCommand::Disable => preferences.enabled = false,
        WakeCommand::Phrase { phrase } => {
            let phrase = phrase.trim();
            if phrase.is_empty() {
                bail!("wake phrase cannot be empty");
            }
            preferences.phrase = Some(phrase.to_owned());
        }
    }
    save_json_atomic(&paths.wake_settings, &preferences)?;
    println!("Wake settings saved: {}", paths.wake_settings.display());
    print_restart_note();
    Ok(())
}

fn command_resources(paths: &AppPaths, command: ResourceCommand) -> Result<()> {
    match command {
        ResourceCommand::List => {
            for resource in [stt_resource(paths), wake_resource(paths)] {
                println!(
                    "{}  {}/{}  {}",
                    resource.id,
                    resource.present,
                    resource.required,
                    if resource.ready { "ready" } else { "not-ready" }
                );
                println!("  {}", resource.root);
                for file in resource.files {
                    println!("  [{}] {}", if file.exists { "x" } else { " " }, file.name);
                }
            }
        }
    }
    Ok(())
}

fn command_permissions(paths: &AppPaths, command: PermissionCommand) -> Result<()> {
    match command {
        PermissionCommand::List => {
            let policy = load_json_or_default::<PermissionOverrideSnapshot>(&paths.permission_policy)?;
            println!("REVISION {}", policy.revision);
            println!("RISK       DECISION   TOOL");
            for tool in TOOL_CATALOG {
                let decision = effective_decision(tool.name, tool.risk, &policy);
                let override_mark = policy
                    .decision_for(tool.name)
                    .filter(|_| matches!(tool.risk, ToolRisk::Moderate))
                    .map(|_| " *")
                    .unwrap_or("");
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
        PermissionCommand::Set { tool, decision } => {
            let definition = TOOL_CATALOG
                .iter()
                .find(|item| item.name == tool)
                .ok_or_else(|| anyhow::anyhow!("unknown tool `{tool}`"))?;
            if !matches!(definition.risk, ToolRisk::Moderate) {
                bail!(
                    "only Moderate tools are user-overridable; `{}` is {}",
                    definition.name,
                    risk_name(definition.risk)
                );
            }
            let mut policy = load_json_or_default::<PermissionOverrideSnapshot>(&paths.permission_policy)?;
            policy.set(definition.name, decision.into());
            save_json_atomic(&paths.permission_policy, &policy)?;
            println!(
                "{} -> {} (revision {})",
                definition.name,
                decision_name(policy.decision_for(definition.name).expect("just set")),
                policy.revision
            );
            print_restart_note();
        }
        PermissionCommand::Clear { tool } => {
            let definition = TOOL_CATALOG
                .iter()
                .find(|item| item.name == tool)
                .ok_or_else(|| anyhow::anyhow!("unknown tool `{tool}`"))?;
            if !matches!(definition.risk, ToolRisk::Moderate) {
                bail!("`{}` does not support a runtime override", definition.name);
            }
            let mut policy = load_json_or_default::<PermissionOverrideSnapshot>(&paths.permission_policy)?;
            policy.clear(definition.name);
            save_json_atomic(&paths.permission_policy, &policy)?;
            println!("{} override cleared (revision {})", definition.name, policy.revision);
            print_restart_note();
        }
    }
    Ok(())
}

fn command_doctor(paths: &AppPaths) -> Result<()> {
    let mut failures = 0usize;
    println!("Assisstant Desktop doctor\n");

    doctor_line("app data path", paths.root.is_absolute(), &paths.root.display().to_string(), &mut failures);

    let binary = resolve_antigravity_binary();
    doctor_line(
        "Antigravity CLI",
        command_available(&binary),
        &binary,
        &mut failures,
    );

    match load_json_or_default::<AiSettings>(&paths.antigravity_settings) {
        Ok(_) => doctor_line("AI settings", true, "valid JSON/default", &mut failures),
        Err(error) => doctor_line("AI settings", false, &error.to_string(), &mut failures),
    }
    match load_json_or_default::<WakePreferences>(&paths.wake_settings) {
        Ok(_) => doctor_line("wake settings", true, "valid JSON/default", &mut failures),
        Err(error) => doctor_line("wake settings", false, &error.to_string(), &mut failures),
    }
    match load_json_or_default::<PermissionOverrideSnapshot>(&paths.permission_policy) {
        Ok(_) => doctor_line("permission policy", true, "valid JSON/default", &mut failures),
        Err(error) => doctor_line("permission policy", false, &error.to_string(), &mut failures),
    }

    let stt = stt_resource(paths);
    doctor_line(
        "Vietnamese STT",
        stt.ready,
        &format!("{}/{} files at {}", stt.present, stt.required, stt.root),
        &mut failures,
    );

    let wake = wake_resource(paths);
    let wake_preferences = load_json_or_default::<WakePreferences>(&paths.wake_settings).unwrap_or_default();
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
        bail!("doctor detected {failures} blocking issue(s)")
    }
}

fn doctor_line(label: &str, ok: bool, detail: &str, failures: &mut usize) {
    if !ok {
        *failures += 1;
    }
    println!("[{}] {:<20} {}", if ok { "OK" } else { "!!" }, label, detail);
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

fn stt_resource(paths: &AppPaths) -> ResourceSummary {
    resource_summary(
        "stt_zipformer_vi",
        "Vietnamese Zipformer STT",
        &paths.stt_model_dir,
        &["encoder.int8.onnx", "decoder.onnx", "joiner.int8.onnx", "tokens.txt"],
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

fn load_json_or_default<T>(path: &Path) -> Result<T>
where
    T: DeserializeOwned + Default,
{
    if !path.is_file() {
        return Ok(T::default());
    }
    let bytes = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("cannot parse {}", path.display()))
}

fn save_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("cannot create {}", parent.display()))?;

    let bytes = serde_json::to_vec_pretty(value)?;
    let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("settings.json");
    let temporary = parent.join(format!(".{file_name}.{}.part", Uuid::new_v4()));
    let backup = parent.join(format!(".{file_name}.{}.bak", Uuid::new_v4()));
    fs::write(&temporary, bytes)
        .with_context(|| format!("cannot write {}", temporary.display()))?;

    let had_existing = path.exists();
    if had_existing {
        if let Err(error) = fs::rename(path, &backup) {
            let _ = fs::remove_file(&temporary);
            return Err(error).with_context(|| format!("cannot stage {}", path.display()));
        }
    }

    if let Err(error) = fs::rename(&temporary, path) {
        if had_existing {
            let _ = fs::rename(&backup, path);
        }
        let _ = fs::remove_file(&temporary);
        return Err(error).with_context(|| format!("cannot commit {}", path.display()));
    }

    if had_existing {
        let _ = fs::remove_file(&backup);
    }
    Ok(())
}

fn default_app_data_dir() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        let local = env::var_os("LOCALAPPDATA")
            .ok_or_else(|| anyhow::anyhow!("LOCALAPPDATA is not available"))?;
        return Ok(PathBuf::from(local).join(APP_IDENTIFIER));
    }

    #[cfg(not(windows))]
    {
        let home = env::var_os("HOME").ok_or_else(|| anyhow::anyhow!("HOME is not available"))?;
        Ok(PathBuf::from(home)
            .join(".local")
            .join("share")
            .join(APP_IDENTIFIER))
    }
}

fn absolute_env_override(name: &str, fallback: PathBuf) -> Result<PathBuf> {
    match env::var_os(name) {
        Some(value) => require_absolute(name, PathBuf::from(value)),
        None => Ok(fallback),
    }
}

fn require_absolute(name: &str, path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path)
    } else {
        bail!("{name} must be an absolute path")
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
            let candidate = PathBuf::from(local_data).join("agy").join("bin").join("agy.exe");
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

fn print_restart_note() {
    println!(
        "Note: this CLI phase persists the shared config safely. A running desktop runtime must be restarted before this setting is guaranteed to be reloaded; live IPC is the next migration phase."
    );
}

fn yes_no(value: bool) -> &'static str {
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
enum TuiTab {
    Dashboard,
    Resources,
    Ai,
    Permissions,
}

impl TuiTab {
    fn next(self) -> Self {
        match self {
            Self::Dashboard => Self::Resources,
            Self::Resources => Self::Ai,
            Self::Ai => Self::Permissions,
            Self::Permissions => Self::Dashboard,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Dashboard => Self::Permissions,
            Self::Resources => Self::Dashboard,
            Self::Ai => Self::Resources,
            Self::Permissions => Self::Ai,
        }
    }
}

struct TuiData {
    status: StatusSnapshot,
    policy: PermissionOverrideSnapshot,
}

impl TuiData {
    fn load(paths: &AppPaths) -> Result<Self> {
        Ok(Self {
            status: StatusSnapshot::load(paths)?,
            policy: load_json_or_default(&paths.permission_policy)?,
        })
    }
}

fn run_tui(paths: AppPaths) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = tui_loop(&mut terminal, &paths);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn tui_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>, paths: &AppPaths) -> Result<()> {
    let mut tab = TuiTab::Dashboard;
    let mut data = TuiData::load(paths)?;

    loop {
        terminal.draw(|frame| render_tui(frame, tab, &data))?;
        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => break,
            KeyCode::Char('r') => data = TuiData::load(paths)?,
            KeyCode::Right | KeyCode::Tab => tab = tab.next(),
            KeyCode::Left | KeyCode::BackTab => tab = tab.previous(),
            KeyCode::Char('1') => tab = TuiTab::Dashboard,
            KeyCode::Char('2') => tab = TuiTab::Resources,
            KeyCode::Char('3') => tab = TuiTab::Ai,
            KeyCode::Char('4') => tab = TuiTab::Permissions,
            _ => {}
        }
    }
    Ok(())
}

fn render_tui(frame: &mut Frame<'_>, tab: TuiTab, data: &TuiData) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(2),
        ])
        .split(frame.area());

    render_tabs(frame, areas[0], tab);
    match tab {
        TuiTab::Dashboard => render_dashboard(frame, areas[1], data),
        TuiTab::Resources => render_resources(frame, areas[1], data),
        TuiTab::Ai => render_ai(frame, areas[1], data),
        TuiTab::Permissions => render_permissions(frame, areas[1], data),
    }
    let footer = Paragraph::new("1-4/←→ switch  r refresh  q/esc quit   |   edits: assistant <section> ...")
        .wrap(Wrap { trim: true });
    frame.render_widget(footer, areas[2]);
}

fn render_tabs(frame: &mut Frame<'_>, area: Rect, active: TuiTab) {
    let labels = [
        ("1 Dashboard", matches!(active, TuiTab::Dashboard)),
        ("2 Resources", matches!(active, TuiTab::Resources)),
        ("3 AI", matches!(active, TuiTab::Ai)),
        ("4 Permissions", matches!(active, TuiTab::Permissions)),
    ];
    let mut spans = Vec::new();
    for (index, (label, selected)) in labels.into_iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw("   "));
        }
        let style = if selected {
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default()
        };
        spans.push(Span::styled(label, style));
    }
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Assisstant Desktop CLI ");
    frame.render_widget(Paragraph::new(Line::from(spans)).block(block), area);
}

fn render_dashboard(frame: &mut Frame<'_>, area: Rect, data: &TuiData) {
    let status = &data.status;
    let lines = vec![
        Line::from(vec![Span::styled("Runtime       ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(runtime_name(status.runtime_running))]),
        Line::from(vec![Span::styled("Antigravity   ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(if status.antigravity_available { "ready" } else { "not-ready" })]),
        Line::from(vec![Span::styled("STT           ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(format!("{}/{} {}", status.stt.present, status.stt.required, if status.stt.ready { "ready" } else { "incomplete" }))]),
        Line::from(vec![Span::styled("Wake          ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(if status.wake_enabled { "enabled" } else { "disabled" })]),
        Line::from(vec![Span::styled("Wake model    ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(format!("{}/{} {}", status.wake.present, status.wake.required, if status.wake.ready { "ready" } else { "incomplete" }))]),
        Line::from(vec![Span::styled("AI model      ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(status.ai_model.as_deref().unwrap_or("default").to_owned())]),
        Line::from(vec![Span::styled("AI effort     ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(status.ai_effort.as_deref().unwrap_or("default").to_owned())]),
        Line::from(""),
        Line::from(format!("Data: {}", status.app_data)),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(" System status "))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_resources(frame: &mut Frame<'_>, area: Rect, data: &TuiData) {
    let mut items = Vec::new();
    for resource in [&data.status.stt, &data.status.wake] {
        items.push(ListItem::new(Line::from(vec![
            Span::styled(resource.label, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("   {}/{} {}", resource.present, resource.required, if resource.ready { "ready" } else { "not-ready" })),
        ])));
        items.push(ListItem::new(resource.root.clone()));
        for file in &resource.files {
            items.push(ListItem::new(format!("  [{}] {}", if file.exists { "x" } else { " " }, file.name)));
        }
        items.push(ListItem::new(""));
    }
    frame.render_widget(
        List::new(items).block(Block::default().borders(Borders::ALL).title(" Local resources ")),
        area,
    );
}

fn render_ai(frame: &mut Frame<'_>, area: Rect, data: &TuiData) {
    let status = &data.status;
    let lines = vec![
        Line::from(vec![Span::styled("CLI       ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(status.antigravity_binary.clone())]),
        Line::from(vec![Span::styled("Available ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(status.antigravity_available.to_string())]),
        Line::from(vec![Span::styled("Model     ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(status.ai_model.as_deref().unwrap_or("default").to_owned())]),
        Line::from(vec![Span::styled("Effort    ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(status.ai_effort.as_deref().unwrap_or("default").to_owned())]),
        Line::from(""),
        Line::from("Commands:"),
        Line::from("  assistant ai models"),
        Line::from("  assistant ai set --model <id> --effort <value>"),
        Line::from("  assistant ai reset"),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(" AI / Antigravity "))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_permissions(frame: &mut Frame<'_>, area: Rect, data: &TuiData) {
    let items = TOOL_CATALOG
        .iter()
        .map(|tool| {
            let decision = effective_decision(tool.name, tool.risk, &data.policy);
            let mark = if matches!(tool.risk, ToolRisk::Moderate)
                && data.policy.decision_for(tool.name).is_some()
            {
                "*"
            } else {
                " "
            };
            ListItem::new(format!(
                "{} {:<10} {:<6} {}",
                mark,
                risk_name(tool.risk),
                decision_name(decision),
                tool.name
            ))
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Permission policy (* Moderate override) "),
        ),
        area,
    );
}

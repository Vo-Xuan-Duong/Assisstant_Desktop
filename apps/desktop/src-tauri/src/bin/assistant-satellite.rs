use std::{env, fs, path::{Path, PathBuf}};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

const APP_IDENTIFIER: &str = "com.voduong.assisstantdesktop";
const SETTINGS_FILE: &str = "satellite.json";
const DEFAULT_BIND: &str = "0.0.0.0:8765";

type CliResult<T> = Result<T, String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SatelliteSettings {
    #[serde(default)]
    enabled: bool,
    #[serde(default = "default_bind")]
    bind: String,
    #[serde(default)]
    token: Option<String>,
}

impl Default for SatelliteSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            bind: default_bind(),
            token: None,
        }
    }
}

fn default_bind() -> String {
    DEFAULT_BIND.to_owned()
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> CliResult<()> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    let data_dir = extract_data_dir(&mut args)?;
    let settings_path = resolve_settings_path(data_dir)?;
    let command = args.first().map(String::as_str).unwrap_or("show");

    match command {
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        "show" | "status" => show(&settings_path),
        "pair" => pair(&settings_path, &args[1..]),
        "enable" => set_enabled(&settings_path, true),
        "disable" => set_enabled(&settings_path, false),
        "revoke" => revoke(&settings_path),
        "bind" => set_bind(&settings_path, &args[1..]),
        other => Err(format!("unknown satellite command `{other}`")),
    }
}

fn print_help() {
    println!(
        r#"Assisstant Desktop Android Voice Satellite pairing helper

USAGE
  assistant-satellite [--data-dir <absolute-path>] <command>

COMMANDS
  show                         Show persisted satellite configuration
  pair [--bind <host:port>]    Create a new pairing token and enable the receiver
  enable                       Enable the receiver using the current token
  disable                      Disable the receiver but keep the pairing token
  revoke                       Disable the receiver and invalidate the pairing token
  bind <host:port>             Change the listener bind address

The running desktop watches settings/satellite.json and normally applies changes
within about one second. A legacy ASSISTANT_VOICE_SATELLITE_TOKEN environment
override takes precedence over this file until that environment override is removed.

ENVIRONMENT
  ASSISTANT_APP_DATA            Override the application data root
"#
    );
}

fn show(path: &Path) -> CliResult<()> {
    let settings = load_settings(path)?;
    println!("Satellite Voice");
    println!("  enabled  {}", settings.enabled);
    println!("  bind     {}", settings.bind);
    println!("  paired   {}", settings.token.is_some());
    println!("  token    {}", masked_token(settings.token.as_deref()));
    println!("  file     {}", path.display());
    println!("  apply    automatic while desktop runtime is running (~1 second)");
    Ok(())
}

fn pair(path: &Path, args: &[String]) -> CliResult<()> {
    let mut settings = load_settings(path)?;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--bind" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--bind requires host:port".to_owned())?;
                settings.bind = validate_bind(value)?;
                index += 2;
            }
            other => return Err(format!("unknown pair option `{other}`")),
        }
    }

    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    settings.enabled = true;
    settings.token = Some(token.clone());
    save_settings(path, &settings)?;

    println!("Android Voice Satellite paired configuration created.");
    println!("  bind   {}", settings.bind);
    println!("  token  {token}");
    println!("  file   {}", path.display());
    println!("If Assisstant Desktop is running, the listener should reload this pairing automatically within about one second.");
    println!("Enter the PC ws:// address and token in the Android app.");
    println!("Treat the token as a local credential; do not publish it in logs/screenshots.");
    Ok(())
}

fn set_enabled(path: &Path, enabled: bool) -> CliResult<()> {
    let mut settings = load_settings(path)?;
    if enabled && settings.token.as_deref().is_none_or(|token| token.len() < 16) {
        return Err("satellite has no valid pairing token; run `assistant-satellite pair` first".into());
    }
    settings.enabled = enabled;
    save_settings(path, &settings)?;
    println!(
        "Satellite receiver {}. Running desktop instances normally apply this within about one second.",
        if enabled { "enabled" } else { "disabled" }
    );
    Ok(())
}

fn revoke(path: &Path) -> CliResult<()> {
    let mut settings = load_settings(path)?;
    settings.enabled = false;
    settings.token = None;
    save_settings(path, &settings)?;
    println!("Satellite pairing revoked. A running desktop normally closes the active satellite session within about one second.");
    Ok(())
}

fn set_bind(path: &Path, args: &[String]) -> CliResult<()> {
    if args.len() != 1 {
        return Err("usage: assistant-satellite bind <host:port>".into());
    }
    let mut settings = load_settings(path)?;
    settings.bind = validate_bind(&args[0])?;
    save_settings(path, &settings)?;
    println!(
        "Satellite bind address set to {}. Running desktop instances normally reload it within about one second.",
        settings.bind
    );
    Ok(())
}

fn validate_bind(value: &str) -> CliResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("bind address cannot be empty".into());
    }
    let (_, port) = value
        .rsplit_once(':')
        .ok_or_else(|| "bind address must use host:port form".to_owned())?;
    let port = port
        .parse::<u16>()
        .map_err(|_| "bind address port must be 1..65535".to_owned())?;
    if port == 0 {
        return Err("bind address port must be 1..65535".into());
    }
    Ok(value.to_owned())
}

fn masked_token(token: Option<&str>) -> String {
    let Some(token) = token else {
        return "not-configured".into();
    };
    if token.len() < 12 {
        return "configured".into();
    }
    format!("{}…{}", &token[..6], &token[token.len() - 4..])
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

fn resolve_settings_path(override_root: Option<PathBuf>) -> CliResult<PathBuf> {
    let root = match override_root {
        Some(path) => require_absolute("application data path", path)?,
        None => default_app_data_dir()?,
    };
    Ok(root.join("settings").join(SETTINGS_FILE))
}

fn default_app_data_dir() -> CliResult<PathBuf> {
    #[cfg(windows)]
    {
        let local = env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| "LOCALAPPDATA is unavailable".to_owned())?;
        return Ok(local.join(APP_IDENTIFIER));
    }

    #[cfg(not(windows))]
    {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| "HOME is unavailable".to_owned())?;
        Ok(home.join(".local").join("share").join(APP_IDENTIFIER))
    }
}

fn require_absolute(name: &str, path: PathBuf) -> CliResult<PathBuf> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(format!("{name} must be an absolute path"))
    }
}

fn load_settings(path: &Path) -> CliResult<SatelliteSettings> {
    if !path.exists() {
        return Ok(SatelliteSettings::default());
    }
    let bytes = fs::read(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))
}

fn save_settings(path: &Path, settings: &SatelliteSettings) -> CliResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;

    let temp = path.with_extension(format!("json.tmp-{}", std::process::id()));
    let bytes = serde_json::to_vec_pretty(settings)
        .map_err(|error| format!("cannot serialize satellite settings: {error}"))?;
    fs::write(&temp, bytes)
        .map_err(|error| format!("cannot write {}: {error}", temp.display()))?;
    if path.exists() {
        fs::remove_file(path)
            .map_err(|error| format!("cannot replace {}: {error}", path.display()))?;
    }
    fs::rename(&temp, path)
        .map_err(|error| format!("cannot promote {}: {error}", path.display()))?;
    Ok(())
}

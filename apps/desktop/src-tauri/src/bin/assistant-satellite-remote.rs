#[path = "assistant_satellite_qr.rs"]
mod qr;

use std::{
    env, fs,
    net::Ipv4Addr,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use serde::{Deserialize, Serialize};
use windows_tools::secret::{
    is_dpapi_text, protect_text_for_current_user, unprotect_text_for_current_user,
};

const APP_IDENTIFIER: &str = "com.voduong.assisstantdesktop";
const SETTINGS_FILE: &str = "satellite.json";
const REMOTE_STATE_FILE: &str = "satellite-tailscale.json";
const REMOTE_STATE_VERSION: u32 = 1;
const TOKEN_ENV: &str = "ASSISTANT_VOICE_SATELLITE_TOKEN";
const TAILSCALE_CLI_ENV: &str = "ASSISTANT_TAILSCALE_CLI";
const MAX_PAIRING_HOST_CHARS: usize = 64;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TailscaleRemoteState {
    version: u32,
    previous_bind: String,
    port: u16,
}

fn default_bind() -> String {
    "0.0.0.0:8765".to_owned()
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
    let remote_state_path = settings_path.with_file_name(REMOTE_STATE_FILE);

    match args.first().map(String::as_str) {
        None | Some("help" | "--help" | "-h") => {
            print_help();
            Ok(())
        }
        Some("tailscale") => tailscale_command(&settings_path, &remote_state_path, &args[1..]),
        Some(other) => Err(format!(
            "unsupported remote provider `{other}`; current provider is `tailscale`"
        )),
    }
}

fn print_help() {
    println!(
        r#"Assisstant Desktop secure remote satellite transport

USAGE
  assistant satellite remote tailscale <command>

COMMANDS
  show        Show Tailscale IP, managed remote state and Serve status
  enable      Move the satellite backend to loopback and expose it with tailnet-only Tailscale Serve TCP
  pair --qr   Print a QR that points Android at this PC's Tailscale IPv4 address
  disable     Disable only the managed Serve TCP listener and restore the previous bind when safe

SECURITY
  This integration uses Tailscale Serve, never Tailscale Funnel. The backend is
  rebound to 127.0.0.1 before it is shared, so the raw satellite port is not opened
  directly to the LAN/public Internet by this command. Existing pairing-token,
  trusted-device, per-device revoke, command-dedup and desktop permission checks
  remain authoritative.

  The Android phone must run Tailscale and be connected to the same permitted
  tailnet. Tailnet access-control policy remains an additional network boundary.

ENVIRONMENT
  ASSISTANT_APP_DATA       Override the application data root
  ASSISTANT_TAILSCALE_CLI  Optional explicit path to tailscale.exe
"#
    );
}

fn tailscale_command(
    settings_path: &Path,
    remote_state_path: &Path,
    args: &[String],
) -> CliResult<()> {
    match args.first().map(String::as_str).unwrap_or("show") {
        "show" | "status" => tailscale_show(settings_path, remote_state_path),
        "enable" => {
            if args.len() != 1 {
                return Err("usage: assistant satellite remote tailscale enable".into());
            }
            tailscale_enable(settings_path, remote_state_path)
        }
        "pair" => tailscale_pair(settings_path, remote_state_path, &args[1..]),
        "disable" | "off" => {
            if args.len() != 1 {
                return Err("usage: assistant satellite remote tailscale disable".into());
            }
            tailscale_disable(settings_path, remote_state_path)
        }
        other => Err(format!(
            "unknown Tailscale remote command `{other}`; use show, enable, pair, or disable"
        )),
    }
}

fn tailscale_show(settings_path: &Path, remote_state_path: &Path) -> CliResult<()> {
    let settings = load_settings(settings_path)?;
    let remote = load_remote_state(remote_state_path)?;
    let ip = tailscale_ipv4().ok();

    println!("Tailscale remote satellite");
    println!("  managed        {}", remote.is_some());
    println!("  backend_bind   {}", settings.bind);
    println!("  paired         {}", settings.token.is_some());
    println!("  tailscale_ip   {}", ip.as_deref().unwrap_or("unavailable"));
    if let Some(state) = &remote {
        println!("  serve_port     {}", state.port);
        println!("  restore_bind   {}", state.previous_bind);
        println!("  state_file     {}", remote_state_path.display());
    }

    println!();
    println!("Tailscale Serve status:");
    match run_tailscale(&["serve", "status"]) {
        Ok(output) => print_output(&output),
        Err(error) => println!("  unavailable: {error}"),
    }

    if remote.is_some() && !is_loopback_bind(&settings.bind) {
        println!("warning: managed Tailscale remote state exists but the satellite backend is no longer bound to loopback");
    }
    if env::var_os(TOKEN_ENV).is_some() {
        println!("warning: {TOKEN_ENV} is set; persisted remote bind changes are not authoritative while that override is active");
    }
    Ok(())
}

fn tailscale_enable(settings_path: &Path, remote_state_path: &Path) -> CliResult<()> {
    reject_token_environment_override()?;

    // Validate Tailscale is installed/connected before changing the local bind.
    let tailscale_ip = tailscale_ipv4()?;
    let mut settings = load_settings(settings_path)?;
    let token = settings
        .token
        .as_deref()
        .map(str::trim)
        .filter(|value| value.len() >= 16)
        .ok_or_else(|| {
            "satellite is not paired; run `assistant satellite pair` or `assistant satellite pair --qr` first"
                .to_owned()
        })?;
    let _ = token;

    let (_, port) = split_bind(&settings.bind)?;
    let existing_state = load_remote_state(remote_state_path)?;
    let previous_bind = existing_state
        .as_ref()
        .map(|state| state.previous_bind.clone())
        .unwrap_or_else(|| settings.bind.clone());
    let original_settings = settings.clone();

    settings.enabled = true;
    settings.bind = format!("127.0.0.1:{port}");
    save_settings(settings_path, &settings)?;

    let serve_port = format!("--tcp={port}");
    let target = format!("tcp://127.0.0.1:{port}");
    if let Err(error) = run_tailscale_success(&["serve", "--bg", &serve_port, &target]) {
        let restore_error = save_settings(settings_path, &original_settings).err();
        return Err(match restore_error {
            Some(restore) => format!(
                "Tailscale Serve setup failed: {error}; additionally failed to restore satellite bind: {restore}"
            ),
            None => format!("Tailscale Serve setup failed: {error}; satellite bind was restored"),
        });
    }

    let state = TailscaleRemoteState {
        version: REMOTE_STATE_VERSION,
        previous_bind,
        port,
    };
    if let Err(error) = save_remote_state(remote_state_path, &state) {
        let _ = run_tailscale_success(&["serve", "--bg", &serve_port, "off"]);
        let _ = save_settings(settings_path, &original_settings);
        return Err(format!(
            "Tailscale Serve was configured but remote state could not be persisted: {error}; rollback was attempted"
        ));
    }

    println!("Tailscale remote satellite enabled.");
    println!("  tailnet_ip    {tailscale_ip}");
    println!("  tailnet_port  {port}");
    println!("  backend       ws://127.0.0.1:{port}");
    println!("  transport     raw WebSocket carried inside the encrypted Tailscale tailnet");
    println!("The LAN firewall rule is not required for this loopback backend.");
    println!("Next: `assistant satellite remote tailscale pair --qr`");
    Ok(())
}

fn tailscale_pair(
    settings_path: &Path,
    remote_state_path: &Path,
    args: &[String],
) -> CliResult<()> {
    if args.iter().any(|arg| arg != "--qr") {
        return Err("usage: assistant satellite remote tailscale pair --qr".into());
    }
    reject_token_environment_override()?;
    let state = load_remote_state(remote_state_path)?.ok_or_else(|| {
        "Tailscale remote mode is not managed yet; run `assistant satellite remote tailscale enable` first"
            .to_owned()
    })?;
    let settings = load_settings(settings_path)?;
    if settings.bind != format!("127.0.0.1:{}", state.port) {
        return Err(format!(
            "satellite backend is not on the managed loopback bind; expected 127.0.0.1:{} but found {}",
            state.port, settings.bind
        ));
    }
    let token = settings
        .token
        .as_deref()
        .map(str::trim)
        .filter(|value| value.len() >= 16)
        .ok_or_else(|| "satellite pairing token is unavailable".to_owned())?;
    let host = tailscale_ipv4()?;
    let uri = pairing_uri(&host, state.port, token)?;
    let rendered = qr::render_terminal_qr(&uri)
        .map_err(|error| format!("cannot render Tailscale pairing QR: {error}"))?;

    println!("Tailscale Android Voice Satellite pairing");
    println!("  phone  ws://{host}:{}", state.port);
    println!("  uri    {uri}");
    println!();
    println!("Scan this QR on the Android phone after Tailscale is connected to the same tailnet:");
    println!("{rendered}");
    println!("Scanning imports the endpoint/token only; Android still requires the normal explicit Connect action.");
    println!("Do not share the QR outside the trusted pairing flow.");
    Ok(())
}

fn tailscale_disable(settings_path: &Path, remote_state_path: &Path) -> CliResult<()> {
    reject_token_environment_override()?;
    let state = load_remote_state(remote_state_path)?.ok_or_else(|| {
        "no Assistant-managed Tailscale remote state exists; nothing was changed".to_owned()
    })?;

    let serve_port = format!("--tcp={}", state.port);
    run_tailscale_success(&["serve", "--bg", &serve_port, "off"])?;

    let mut settings = load_settings(settings_path)?;
    let managed_loopback = format!("127.0.0.1:{}", state.port);
    if settings.bind == managed_loopback {
        settings.bind = state.previous_bind.clone();
        save_settings(settings_path, &settings)?;
        println!("Restored satellite bind to {}.", settings.bind);
    } else {
        println!(
            "Satellite bind is {}, not the managed {}; leaving the user-modified bind unchanged.",
            settings.bind, managed_loopback
        );
    }

    fs::remove_file(remote_state_path).map_err(|error| {
        format!(
            "Tailscale Serve was disabled but cannot remove remote state {}: {error}",
            remote_state_path.display()
        )
    })?;
    println!("Tailscale remote satellite disabled. No Funnel/public exposure was configured by this integration.");
    Ok(())
}

fn tailscale_ipv4() -> CliResult<String> {
    let output = run_tailscale(&["ip", "-4"])?;
    if !output.status.success() {
        return Err(format_command_failure("tailscale ip -4", &output));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let address = stdout
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .ok_or_else(|| "Tailscale did not report an IPv4 address; make sure this PC is connected to a tailnet".to_owned())?;
    let ip = address
        .parse::<Ipv4Addr>()
        .map_err(|_| format!("Tailscale returned an invalid IPv4 address: {address}"))?;
    let octets = ip.octets();
    if octets[0] != 100 || !(64..=127).contains(&octets[1]) {
        return Err(format!(
            "Tailscale IPv4 `{ip}` is outside the expected 100.64.0.0/10 CGNAT range"
        ));
    }
    Ok(ip.to_string())
}

fn run_tailscale_success(args: &[&str]) -> CliResult<()> {
    let output = run_tailscale(args)?;
    print_output(&output);
    if output.status.success() {
        Ok(())
    } else {
        Err(format_command_failure("tailscale", &output))
    }
}

fn run_tailscale(args: &[&str]) -> CliResult<Output> {
    let executable = env::var_os(TAILSCALE_CLI_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("tailscale"));
    Command::new(&executable)
        .args(args)
        .output()
        .map_err(|error| {
            format!(
                "cannot launch Tailscale CLI `{}`: {error}. Install/connect Tailscale or set {TAILSCALE_CLI_ENV} to tailscale.exe",
                executable.display()
            )
        })
}

fn print_output(output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stdout.trim().is_empty() {
        print!("{stdout}");
        if !stdout.ends_with('\n') {
            println!();
        }
    }
    if !stderr.trim().is_empty() {
        eprint!("{stderr}");
        if !stderr.ends_with('\n') {
            eprintln!();
        }
    }
}

fn format_command_failure(command: &str, output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if !stderr.trim().is_empty() {
        stderr.trim()
    } else {
        stdout.trim()
    };
    if detail.is_empty() {
        format!("{command} failed with {}", output.status)
    } else {
        format!("{command} failed with {}: {detail}", output.status)
    }
}

fn reject_token_environment_override() -> CliResult<()> {
    if env::var_os(TOKEN_ENV).is_some() {
        return Err(format!(
            "{TOKEN_ENV} is set. Remove the legacy token override before enabling managed Tailscale remote mode so the loopback bind in satellite.json is authoritative."
        ));
    }
    Ok(())
}

fn split_bind(bind: &str) -> CliResult<(&str, u16)> {
    let (host, port) = bind
        .trim()
        .rsplit_once(':')
        .ok_or_else(|| "bind address must use host:port form".to_owned())?;
    let port = port
        .parse::<u16>()
        .map_err(|_| "bind address port must be 1..65535".to_owned())?;
    if port == 0 {
        return Err("bind address port must be 1..65535".into());
    }
    Ok((host.trim(), port))
}

fn is_loopback_bind(bind: &str) -> bool {
    split_bind(bind)
        .map(|(host, _)| host == "127.0.0.1" || host.eq_ignore_ascii_case("localhost"))
        .unwrap_or(false)
}

fn pairing_uri(host: &str, port: u16, token: &str) -> CliResult<String> {
    let host = validate_pairing_host(host)?;
    if token.len() < 16 || !token.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err("pairing token is not a valid hexadecimal credential".into());
    }
    Ok(format!("assd://p?h={host}&p={port}&t={token}"))
}

fn validate_pairing_host(value: &str) -> CliResult<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > MAX_PAIRING_HOST_CHARS {
        return Err(format!(
            "pairing host must contain 1..={MAX_PAIRING_HOST_CHARS} characters"
        ));
    }
    let ip = value
        .parse::<Ipv4Addr>()
        .map_err(|_| "Tailscale remote pairing currently requires the PC's Tailscale IPv4 address".to_owned())?;
    if ip.is_unspecified() || ip.is_loopback() {
        return Err("pairing host cannot be loopback/unspecified".into());
    }
    Ok(ip.to_string())
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
        Some(path) if path.is_absolute() => path,
        Some(path) => {
            return Err(format!(
                "application data path must be absolute: {}",
                path.display()
            ));
        }
        None => env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| "LOCALAPPDATA is unavailable".to_owned())?
            .join(APP_IDENTIFIER),
    };
    Ok(root.join("settings").join(SETTINGS_FILE))
}

fn load_settings(path: &Path) -> CliResult<SatelliteSettings> {
    if !path.exists() {
        return Ok(SatelliteSettings::default());
    }
    let bytes = fs::read(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let mut settings: SatelliteSettings = serde_json::from_slice(&bytes)
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))?;
    if let Some(value) = settings.token.take() {
        let value = value.trim();
        let token = if is_dpapi_text(value) {
            unprotect_text_for_current_user(value).map_err(|error| {
                format!(
                    "cannot decrypt satellite pairing token in {} for the current Windows user: {error}",
                    path.display()
                )
            })?
        } else {
            value.to_owned()
        };
        if !token.is_empty() {
            settings.token = Some(token);
        }
    }
    Ok(settings)
}

fn save_settings(path: &Path, settings: &SatelliteSettings) -> CliResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;

    let mut persisted = settings.clone();
    if let Some(token) = settings
        .token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        persisted.token = Some(protect_text_for_current_user(token).map_err(|error| {
            format!("cannot protect satellite pairing token with Windows DPAPI: {error}")
        })?);
    } else {
        persisted.token = None;
    }

    save_json_atomic(path, &persisted, "satellite settings")
}

fn load_remote_state(path: &Path) -> CliResult<Option<TailscaleRemoteState>> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let state: TailscaleRemoteState = serde_json::from_slice(&bytes)
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))?;
    if state.version != REMOTE_STATE_VERSION {
        return Err(format!(
            "unsupported Tailscale remote state version {} in {}",
            state.version,
            path.display()
        ));
    }
    if state.port == 0 {
        return Err(format!("invalid Tailscale remote port in {}", path.display()));
    }
    Ok(Some(state))
}

fn save_remote_state(path: &Path, state: &TailscaleRemoteState) -> CliResult<()> {
    save_json_atomic(path, state, "Tailscale remote state")
}

fn save_json_atomic<T: Serialize>(path: &Path, value: &T, label: &str) -> CliResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    let temp = path.with_extension(format!("json.tmp-{}", std::process::id()));
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("cannot serialize {label}: {error}"))?;
    fs::write(&temp, bytes)
        .map_err(|error| format!("cannot write {}: {error}", temp.display()))?;
    if path.exists() {
        fs::remove_file(path)
            .map_err(|error| format!("cannot replace {}: {error}", path.display()))?;
    }
    fs::rename(&temp, path)
        .map_err(|error| format!("cannot promote {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_bind_detection_is_exact() {
        assert!(is_loopback_bind("127.0.0.1:8765"));
        assert!(is_loopback_bind("localhost:8765"));
        assert!(!is_loopback_bind("0.0.0.0:8765"));
    }

    #[test]
    fn tailscale_pairing_accepts_cgnat_ipv4() {
        let token = "0123456789abcdef".repeat(4);
        let uri = pairing_uri("100.101.102.103", 8765, &token).unwrap();
        assert_eq!(
            uri,
            format!("assd://p?h=100.101.102.103&p=8765&t={token}")
        );
    }
}

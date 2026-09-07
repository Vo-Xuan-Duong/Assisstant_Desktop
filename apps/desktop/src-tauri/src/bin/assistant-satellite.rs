#[path = "assistant_satellite_qr.rs"]
mod qr;

use std::{
    env, fs,
    net::{Ipv4Addr, UdpSocket},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

const APP_IDENTIFIER: &str = "com.voduong.assisstantdesktop";
const SETTINGS_FILE: &str = "satellite.json";
const DEVICES_FILE: &str = "satellite-devices.json";
const REVOKED_DIR: &str = "satellite-revoked";
const DEFAULT_BIND: &str = "0.0.0.0:8765";
const MAX_DEVICE_ID_CHARS: usize = 128;
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DeviceRegistry {
    #[serde(default)]
    devices: Vec<TrustedDevice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrustedDevice {
    id: String,
    name: String,
    first_seen_unix: u64,
    last_seen_unix: u64,
    #[serde(default)]
    revoked: bool,
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
    let devices_path = settings_path.with_file_name(DEVICES_FILE);
    let revoked_dir = settings_path
        .parent()
        .ok_or_else(|| "satellite settings path has no parent".to_owned())?
        .join(REVOKED_DIR);
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
        "devices" => list_devices(&devices_path, &revoked_dir),
        "revoke-device" => {
            set_device_revoked(&devices_path, &revoked_dir, &args[1..], true)
        }
        "allow-device" => {
            set_device_revoked(&devices_path, &revoked_dir, &args[1..], false)
        }
        other => Err(format!("unknown satellite command `{other}`")),
    }
}

fn print_help() {
    println!(
        r#"Assisstant Desktop Android Voice Satellite pairing helper

USAGE
  assistant-satellite [--data-dir <absolute-path>] <command>

COMMANDS
  show                                      Show persisted satellite configuration
  pair [--bind <host:port>]                 Create a new pairing token and enable the receiver
  pair --qr [--host <LAN-IP>] [--bind ...]  Create pairing and print a local terminal QR code
  enable                                    Enable the receiver using the current token
  disable                                   Disable the receiver but keep the pairing token
  revoke                                    Disable the receiver and invalidate the shared pairing token
  bind <host:port>                          Change the listener bind address
  devices                                   List known Android satellite devices
  revoke-device <device-id>                 Revoke one Android device without rotating the shared token
  allow-device <device-id>                  Re-enable a previously revoked Android device

QR pairing never sends the token to a web service. The QR is generated locally and
encodes a compact `assd://p` deep link. When --host is omitted and the listener binds
to 0.0.0.0, the helper attempts to resolve the PC's routed LAN IPv4 address. Use
--host explicitly when the machine has multiple adapters or the detected address is
not reachable from the phone.

The running desktop watches settings/satellite.json and normally applies listener
changes within about one second. Trusted-device revocation is checked by an active
phone session about once per second as well.

A legacy ASSISTANT_VOICE_SATELLITE_TOKEN environment override takes precedence over
satellite.json until that environment override is removed. Per-device revocation
still applies to authenticated Android clients.

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
    let mut show_qr = false;
    let mut pairing_host = None::<String>;
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
            "--qr" => {
                show_qr = true;
                index += 1;
            }
            "--host" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--host requires a LAN IPv4 address or hostname".to_owned())?;
                pairing_host = Some(validate_pairing_host(value)?);
                index += 2;
            }
            other => return Err(format!("unknown pair option `{other}`")),
        }
    }

    if pairing_host.is_some() && !show_qr {
        return Err("--host is only used with `pair --qr`".into());
    }

    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let qr_output = if show_qr {
        let (_, port) = split_bind(&settings.bind)?;
        let host = match pairing_host {
            Some(host) => host,
            None => pairing_host_from_bind(&settings.bind)
                .or_else(discover_lan_ipv4)
                .ok_or_else(|| {
                    "cannot determine a phone-reachable LAN address; rerun with `pair --qr --host <PC-LAN-IP>`"
                        .to_owned()
                })?,
        };
        let uri = pairing_uri(&host, port, &token)?;
        let qr = qr::render_terminal_qr(&uri).map_err(|error| {
            format!("cannot render pairing QR: {error}; use a shorter IPv4 host via --host")
        })?;
        Some((host, port, uri, qr))
    } else {
        None
    };

    settings.enabled = true;
    settings.token = Some(token.clone());
    save_settings(path, &settings)?;

    println!("Android Voice Satellite paired configuration created.");
    println!("  bind   {}", settings.bind);
    println!("  token  {token}");
    println!("  file   {}", path.display());
    println!("If Assisstant Desktop is running, the listener should reload this pairing automatically within about one second.");

    if let Some((host, port, uri, qr)) = qr_output {
        println!("  phone  ws://{host}:{port}");
        println!("  uri    {uri}");
        println!();
        println!("Scan this QR with the Android camera/QR scanner. Windows Terminal or another ANSI-capable terminal is recommended:");
        println!("{qr}");
        println!("Scanning only imports the pairing data; the Android app still requires an explicit Connect tap.");
    } else {
        println!("Enter the PC ws:// address and token in the Android app, or use `assistant-satellite pair --qr` next time.");
    }

    println!("Treat the token and QR as local credentials; do not publish them in logs/screenshots.");
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

fn pairing_host_from_bind(bind: &str) -> Option<String> {
    let (host, _) = split_bind(bind).ok()?;
    if host.is_empty()
        || host == "0.0.0.0"
        || host == "::"
        || host == "[::]"
        || host.eq_ignore_ascii_case("localhost")
        || host == "127.0.0.1"
    {
        return None;
    }
    validate_pairing_host(host).ok()
}

fn discover_lan_ipv4() -> Option<String> {
    // UDP connect performs route selection without sending application data.
    // It works on ordinary LAN/default-route setups even when no packet is sent.
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    let address = socket.local_addr().ok()?;
    let std::net::IpAddr::V4(ip) = address.ip() else {
        return None;
    };
    if ip.is_unspecified() || ip.is_loopback() {
        return None;
    }
    Some(ip.to_string())
}

fn validate_pairing_host(value: &str) -> CliResult<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > MAX_PAIRING_HOST_CHARS {
        return Err(format!(
            "pairing host must contain 1..={MAX_PAIRING_HOST_CHARS} characters"
        ));
    }

    if let Ok(ip) = value.parse::<Ipv4Addr>() {
        if ip.is_unspecified() || ip.is_loopback() {
            return Err("pairing host must be reachable from the Android phone, not loopback/unspecified".into());
        }
        return Ok(ip.to_string());
    }

    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.'))
        || value.starts_with('.')
        || value.ends_with('.')
        || value.contains("..")
    {
        return Err("pairing hostname contains unsupported characters".into());
    }
    Ok(value.to_ascii_lowercase())
}

fn pairing_uri(host: &str, port: u16, token: &str) -> CliResult<String> {
    let host = validate_pairing_host(host)?;
    if token.len() < 16 || !token.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err("pairing token is not a valid hexadecimal credential".into());
    }
    Ok(format!("assd://p?h={host}&p={port}&t={token}"))
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

fn list_devices(path: &Path, revoked_dir: &Path) -> CliResult<()> {
    let mut registry = load_device_registry(path)?;
    registry.devices.sort_by(|left, right| {
        right
            .last_seen_unix
            .cmp(&left.last_seen_unix)
            .then_with(|| left.id.cmp(&right.id))
    });

    if registry.devices.is_empty() {
        println!("No Android satellite devices have authenticated yet.");
        println!("registry  {}", path.display());
        return Ok(());
    }

    println!("Android Voice Satellite devices");
    for device in registry.devices {
        let revoked = device.revoked || revocation_marker(revoked_dir, &device.id).is_file();
        println!(
            "  [{}] {}  {}",
            if revoked { "revoked" } else { "trusted" },
            device.id,
            device.name
        );
        println!("      first_seen_unix  {}", device.first_seen_unix);
        println!("      last_seen_unix   {}", device.last_seen_unix);
    }
    println!("registry  {}", path.display());
    Ok(())
}

fn set_device_revoked(
    path: &Path,
    revoked_dir: &Path,
    args: &[String],
    revoked: bool,
) -> CliResult<()> {
    if args.len() != 1 {
        return Err(if revoked {
            "usage: assistant-satellite revoke-device <device-id>".into()
        } else {
            "usage: assistant-satellite allow-device <device-id>".into()
        });
    }
    let device_id = validate_device_id(&args[0])?;
    let mut registry = load_device_registry(path)?;
    let device = registry
        .devices
        .iter_mut()
        .find(|device| device.id == device_id)
        .ok_or_else(|| format!("unknown satellite device `{device_id}`"))?;
    let name = device.name.clone();
    let marker = revocation_marker(revoked_dir, &device_id);

    if revoked {
        fs::create_dir_all(revoked_dir)
            .map_err(|error| format!("cannot create {}: {error}", revoked_dir.display()))?;
        fs::write(&marker, b"revoked\n")
            .map_err(|error| format!("cannot write {}: {error}", marker.display()))?;
        device.revoked = true;
        save_device_registry(path, &registry)?;
        println!("Revoked satellite device `{device_id}` ({name}).");
        println!("An active connection from this device should be closed by the desktop within about one second.");
    } else {
        if marker.exists() {
            fs::remove_file(&marker)
                .map_err(|error| format!("cannot remove {}: {error}", marker.display()))?;
        }
        device.revoked = false;
        save_device_registry(path, &registry)?;
        println!("Allowed satellite device `{device_id}` ({name}) again.");
    }
    Ok(())
}

fn revocation_marker(revoked_dir: &Path, device_id: &str) -> PathBuf {
    revoked_dir.join(format!("{device_id}.revoked"))
}

fn validate_bind(value: &str) -> CliResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("bind address cannot be empty".into());
    }
    let (_, _) = split_bind(value)?;
    Ok(value.to_owned())
}

fn validate_device_id(value: &str) -> CliResult<String> {
    let value = value.trim();
    let chars = value.chars().count();
    if !(8..=MAX_DEVICE_ID_CHARS).contains(&chars) {
        return Err(format!(
            "device-id must contain 8..={MAX_DEVICE_ID_CHARS} characters"
        ));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':'))
    {
        return Err("device-id contains unsupported characters".into());
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

fn load_device_registry(path: &Path) -> CliResult<DeviceRegistry> {
    if !path.exists() {
        return Ok(DeviceRegistry::default());
    }
    let bytes = fs::read(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))
}

fn save_device_registry(path: &Path, registry: &DeviceRegistry) -> CliResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;

    let temp = path.with_extension(format!("json.tmp-{}", std::process::id()));
    let bytes = serde_json::to_vec_pretty(registry)
        .map_err(|error| format!("cannot serialize satellite device registry: {error}"))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_uri_is_compact_and_query_safe() {
        let token = "0123456789abcdef".repeat(4);
        let uri = pairing_uri("192.168.1.20", 8765, &token).expect("valid pairing URI");
        assert_eq!(
            uri,
            format!("assd://p?h=192.168.1.20&p=8765&t={token}")
        );
    }

    #[test]
    fn pairing_host_rejects_loopback_and_query_injection() {
        assert!(validate_pairing_host("127.0.0.1").is_err());
        assert!(validate_pairing_host("pc.local&t=stolen").is_err());
    }
}

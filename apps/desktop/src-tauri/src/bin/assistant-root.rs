use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

const CORE_OVERRIDE_ENV: &str = "ASSISTANT_CORE_CLI";
const SATELLITE_OVERRIDE_ENV: &str = "ASSISTANT_SATELLITE_CLI";
const SATELLITE_REMOTE_OVERRIDE_ENV: &str = "ASSISTANT_SATELLITE_REMOTE_CLI";
const TTS_OVERRIDE_ENV: &str = "ASSISTANT_TTS_CLI";

fn main() {
    match run() {
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<ExitStatus, String> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    let command_index = first_command_index(&args)?;
    let command = command_index.and_then(|index| args.get(index));

    if command.is_some_and(|value| value == OsStr::new("satellite")) {
        let index = command_index.expect("satellite command index must exist");
        if args.get(index + 1).is_some_and(|value| value == OsStr::new("remote")) {
            let mut forwarded = args;
            forwarded.remove(index + 1);
            forwarded.remove(index);
            return run_cli(
                resolve_cli(
                    SATELLITE_REMOTE_OVERRIDE_ENV,
                    "assistant-satellite-remote",
                )?,
                &forwarded,
                "satellite remote CLI",
            );
        }
        return run_namespaced_cli(
            args,
            index,
            SATELLITE_OVERRIDE_ENV,
            "assistant-satellite",
            "satellite CLI",
        );
    }

    if command.is_some_and(|value| value == OsStr::new("tts")) {
        return run_namespaced_cli(
            args,
            command_index.expect("tts command index must exist"),
            TTS_OVERRIDE_ENV,
            "assistant-tts",
            "TTS CLI",
        );
    }

    let core = resolve_cli(CORE_OVERRIDE_ENV, "assistant-core")?;
    let status = run_cli(core, &args, "management CLI")?;
    if status.success()
        && command.is_some_and(|value| {
            value == OsStr::new("help") || value == OsStr::new("--help") || value == OsStr::new("-h")
        })
    {
        print_extension_help_hint();
    }
    Ok(status)
}

fn run_namespaced_cli(
    mut args: Vec<OsString>,
    command_index: usize,
    override_env: &str,
    stem: &str,
    label: &str,
) -> Result<ExitStatus, String> {
    args.remove(command_index);
    run_cli(resolve_cli(override_env, stem)?, &args, label)
}

fn run_cli(path: PathBuf, args: &[OsString], label: &str) -> Result<ExitStatus, String> {
    Command::new(&path)
        .args(args)
        .status()
        .map_err(|error| format!("cannot launch {label} {}: {error}", path.display()))
}

fn print_extension_help_hint() {
    println!(
        r#"
  satellite show                           Show Android voice satellite configuration
  satellite doctor                         Diagnose pairing, bind, trust and credential storage
  satellite pair --qr [--host <LAN-IP>]    Pair a phone with a local terminal QR
  satellite devices                        List trusted/revoked Android devices
  satellite revoke-device <device-id>      Revoke one Android device
  satellite allow-device <device-id>       Allow a previously revoked device
  satellite firewall show                  Inspect the named Windows Firewall rule
  satellite firewall install               Install Private+LocalSubnet TCP rule
  satellite firewall remove                Remove only the Assistant satellite firewall rule
  satellite remote tailscale show          Show managed tailnet-only remote transport
  satellite remote tailscale enable        Bind backend to loopback and enable Tailscale Serve TCP
  satellite remote tailscale pair --qr     Pair Android using the PC's Tailscale IPv4
  satellite remote tailscale disable       Disable managed Serve and restore previous bind
  satellite help                           Show the complete satellite command surface

  tts voices [--json]                      List installed Windows SAPI voices
  tts show [--json]                        Show preferred VI/EN voice tokens
  tts set <vi|en> <index|voice-id>         Select a preferred installed voice
  tts clear <vi|en|all>                    Return to locale/default voice fallback
  tts help                                 Show the complete TTS command surface
"#
    );
}

fn first_command_index(args: &[OsString]) -> Result<Option<usize>, String> {
    let mut index = 0usize;
    while index < args.len() {
        let value = args[index].to_string_lossy();
        if value == "--data-dir" {
            if index + 1 >= args.len() {
                return Err("--data-dir requires an absolute path".to_owned());
            }
            index += 2;
            continue;
        }
        if value.starts_with("--data-dir=") {
            index += 1;
            continue;
        }
        return Ok(Some(index));
    }
    Ok(None)
}

fn resolve_cli(override_env: &str, stem: &str) -> Result<PathBuf, String> {
    if let Some(path) = env::var_os(override_env).map(PathBuf::from) {
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "{override_env} points to a missing file: {}",
            path.display()
        ));
    }

    let current = env::current_exe()
        .map_err(|error| format!("cannot resolve assistant executable path: {error}"))?;
    let directory = current
        .parent()
        .ok_or_else(|| format!("assistant executable has no parent directory: {}", current.display()))?;

    for candidate in exact_candidates(directory, &current, stem) {
        if candidate.is_file() && !same_path(&candidate, &current) {
            return Ok(candidate);
        }
    }

    let prefix = format!("{stem}-");
    let mut matches = fs::read_dir(directory)
        .map_err(|error| format!("cannot inspect {} for `{stem}` sidecar: {error}", directory.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            let file_stem = path.file_stem().and_then(OsStr::to_str).unwrap_or_default();
            file_stem.starts_with(&prefix)
                && executable_extension_matches(path)
                && !same_path(path, &current)
        })
        .collect::<Vec<_>>();
    matches.sort();

    match matches.as_slice() {
        [path] => Ok(path.clone()),
        [] => Err(format!(
            "cannot find `{stem}` beside {}. Re-run desktop sidecar staging or reinstall the Windows bundle.",
            current.display()
        )),
        _ => Err(format!(
            "multiple `{stem}` sidecar candidates exist beside {}; remove stale staged binaries",
            current.display()
        )),
    }
}

fn exact_candidates(directory: &Path, current: &Path, stem: &str) -> Vec<PathBuf> {
    let mut result = vec![directory.join(executable_name(stem))];

    if let Some(current_stem) = current.file_stem().and_then(OsStr::to_str) {
        if let Some(target_suffix) = current_stem.strip_prefix("assistant-") {
            if !target_suffix.is_empty() {
                result.push(directory.join(executable_name(&format!("{stem}-{target_suffix}"))));
            }
        }
    }

    result
}

fn executable_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_owned()
    }
}

fn executable_extension_matches(path: &Path) -> bool {
    if cfg!(windows) {
        path.extension()
            .and_then(OsStr::to_str)
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    } else {
        path.extension().is_none()
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    left == right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_data_dir_is_skipped_before_namespaced_command() {
        let args = vec![
            OsString::from("--data-dir"),
            OsString::from("C:/AssistantData"),
            OsString::from("tts"),
            OsString::from("voices"),
        ];
        assert_eq!(first_command_index(&args).unwrap(), Some(2));
    }

    #[test]
    fn ordinary_commands_are_left_for_core_cli() {
        let args = vec![OsString::from("status"), OsString::from("--json")];
        assert_eq!(first_command_index(&args).unwrap(), Some(0));
    }

    #[test]
    fn missing_data_dir_value_is_rejected() {
        let args = vec![OsString::from("--data-dir")];
        assert!(first_command_index(&args).is_err());
    }
}

use std::{env, path::PathBuf};

use serde::Serialize;
use voice_runtime::tts::{
    SapiVoiceInfo, TtsVoicePreferences, enumerate_windows_sapi_voices, load_voice_preferences,
    save_voice_preferences,
};

const APP_IDENTIFIER: &str = "com.voduong.assisstantdesktop";
const APP_DATA_ENV: &str = "ASSISTANT_APP_DATA";
const TTS_SETTINGS_ENV: &str = "ASSISTANT_TTS_SETTINGS_PATH";

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
    let settings_path = resolve_settings_path(data_dir)?;

    match args.first().map(String::as_str) {
        None | Some("help" | "--help" | "-h") => {
            print_help();
            Ok(())
        }
        Some("voices") => command_voices(args.iter().skip(1).any(|arg| arg == "--json")),
        Some("show") => command_show(
            &settings_path,
            args.iter().skip(1).any(|arg| arg == "--json"),
        ),
        Some("set") => command_set(&settings_path, &args[1..]),
        Some("clear") => command_clear(&settings_path, &args[1..]),
        Some(other) => Err(format!(
            "unknown TTS command `{other}`. Run `assistant tts help`."
        )),
    }
}

fn print_help() {
    println!(
        r#"Assisstant Desktop Windows SAPI voice management

USAGE
  assistant tts voices [--json]
  assistant tts show [--json]
  assistant tts set <vi|en> <voice-index|voice-id>
  assistant tts clear <vi|en|all>

COMMANDS
  voices       List installed SAPI voices with stable token ids
  show         Show preferred Vietnamese/English voice ids
  set          Persist one preferred voice; takes effect on the next utterance
  clear        Return one or both languages to locale/default fallback

EXAMPLES
  assistant tts voices
  assistant tts set vi 2
  assistant tts set en "HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Speech..."
  assistant tts clear vi

The numeric index is only a convenience for the current `voices` output. The
persisted setting always stores the stable SAPI token id, never the list index.
The runtime preference file is a small versioned `tts.conf`; `--json` affects
CLI output only and does not change the persisted format.
"#
    );
}

fn command_voices(output_json: bool) -> CliResult<()> {
    let voices = voices()?;
    if output_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&voices)
                .map_err(|error| format!("cannot encode SAPI voice list: {error}"))?
        );
        return Ok(());
    }

    if voices.is_empty() {
        println!("No Windows SAPI voices are installed.");
        return Ok(());
    }

    println!("Installed Windows SAPI voices:");
    for (index, voice) in voices.iter().enumerate() {
        println!(
            "  [{index}] {}  language={}\n      {}",
            voice.name,
            voice.language.as_deref().unwrap_or("unknown"),
            voice.id
        );
    }
    Ok(())
}

#[derive(Serialize)]
struct PreferencesView<'a> {
    path: String,
    vietnamese_voice_id: Option<&'a str>,
    english_voice_id: Option<&'a str>,
}

fn command_show(settings_path: &PathBuf, output_json: bool) -> CliResult<()> {
    let preferences = load_voice_preferences(settings_path).map_err(|error| error.to_string())?;
    if output_json {
        let view = PreferencesView {
            path: settings_path.display().to_string(),
            vietnamese_voice_id: preferences.vietnamese_voice_id.as_deref(),
            english_voice_id: preferences.english_voice_id.as_deref(),
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&view)
                .map_err(|error| format!("cannot encode TTS settings: {error}"))?
        );
        return Ok(());
    }

    println!("Windows SAPI preferences");
    println!("  Settings   {}", settings_path.display());
    println!(
        "  Vietnamese {}",
        preferences
            .vietnamese_voice_id
            .as_deref()
            .unwrap_or("automatic")
    );
    println!(
        "  English    {}",
        preferences
            .english_voice_id
            .as_deref()
            .unwrap_or("automatic")
    );
    Ok(())
}

fn command_set(settings_path: &PathBuf, args: &[String]) -> CliResult<()> {
    if args.len() != 2 {
        return Err("usage: assistant tts set <vi|en> <voice-index|voice-id>".into());
    }
    let language = normalize_language(&args[0])?;
    let installed = voices()?;
    let voice = resolve_voice(&installed, &args[1])?;
    let mut preferences =
        load_voice_preferences(settings_path).map_err(|error| error.to_string())?;

    match language {
        "vi" => preferences.vietnamese_voice_id = Some(voice.id.clone()),
        "en" => preferences.english_voice_id = Some(voice.id.clone()),
        _ => unreachable!(),
    }
    save_voice_preferences(settings_path, &preferences).map_err(|error| error.to_string())?;

    println!(
        "Preferred {} voice set to: {}\nToken: {}\nThis applies on the next spoken response.",
        language.to_uppercase(),
        voice.name,
        voice.id
    );
    if let Some(attribute) = voice.language.as_deref() {
        let expected = if language == "vi" { "42A" } else { "409" };
        if !language_attribute_contains(attribute, expected) {
            println!(
                "warning: this voice reports Language={attribute}; expected locale token {expected}. SAPI may still reject or override it for that language."
            );
        }
    }
    Ok(())
}

fn command_clear(settings_path: &PathBuf, args: &[String]) -> CliResult<()> {
    if args.len() != 1 {
        return Err("usage: assistant tts clear <vi|en|all>".into());
    }
    let mut preferences =
        load_voice_preferences(settings_path).map_err(|error| error.to_string())?;
    match args[0].to_ascii_lowercase().as_str() {
        "vi" | "vietnamese" => preferences.vietnamese_voice_id = None,
        "en" | "english" => preferences.english_voice_id = None,
        "all" => preferences = TtsVoicePreferences::default(),
        _ => return Err("language must be `vi`, `en`, or `all`".into()),
    }
    save_voice_preferences(settings_path, &preferences).map_err(|error| error.to_string())?;
    println!(
        "TTS voice preference cleared; locale/default fallback applies on the next utterance."
    );
    Ok(())
}

fn voices() -> CliResult<Vec<SapiVoiceInfo>> {
    enumerate_windows_sapi_voices().map_err(|error| error.to_string())
}

fn resolve_voice<'a>(voices: &'a [SapiVoiceInfo], selector: &str) -> CliResult<&'a SapiVoiceInfo> {
    if let Ok(index) = selector.parse::<usize>() {
        return voices.get(index).ok_or_else(|| {
            format!("voice index {index} is out of range; run `assistant tts voices`")
        });
    }
    voices
        .iter()
        .find(|voice| voice.id == selector)
        .ok_or_else(|| "SAPI voice token id is not currently installed".to_owned())
}

fn normalize_language(value: &str) -> CliResult<&'static str> {
    match value.to_ascii_lowercase().as_str() {
        "vi" | "vietnamese" => Ok("vi"),
        "en" | "english" => Ok("en"),
        _ => Err("language must be `vi` or `en`".into()),
    }
}

fn language_attribute_contains(attribute: &str, expected: &str) -> bool {
    attribute
        .split(';')
        .map(str::trim)
        .any(|value| value.eq_ignore_ascii_case(expected))
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
    Ok(result.or_else(|| env::var_os(APP_DATA_ENV).map(PathBuf::from)))
}

fn resolve_settings_path(data_dir: Option<PathBuf>) -> CliResult<PathBuf> {
    if let Some(path) = env::var_os(TTS_SETTINGS_ENV).map(PathBuf::from) {
        if path.is_absolute() {
            return Ok(path);
        }
        return Err(format!("{TTS_SETTINGS_ENV} must be an absolute path"));
    }

    let root = match data_dir {
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
    Ok(root.join("settings").join("tts.conf"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_attribute_can_contain_multiple_langids() {
        assert!(language_attribute_contains("409;809", "409"));
        assert!(language_attribute_contains("42A", "42a"));
        assert!(!language_attribute_contains("409", "42A"));
    }
}

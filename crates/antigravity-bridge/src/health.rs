use std::io::ErrorKind;

use tokio::process::Command;

use crate::AntigravityConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeFailureKind {
    Authentication,
    Quota,
    Permission,
    Model,
    Transport,
    Process,
    Protocol,
    InvalidInput,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliHealth {
    Available { detail: Option<String> },
    Missing,
    Unhealthy { message: String },
}

pub async fn probe_cli(config: &AntigravityConfig) -> CliHealth {
    match Command::new(&config.binary).arg("--help").output().await {
        Ok(output) if output.status.success() => {
            let detail = match &config.model {
                Some(model) => format!("Google Antigravity CLI · Model: {model}"),
                None => "Google Antigravity CLI · Gemini (Default)".to_string(),
            };

            CliHealth::Available {
                detail: Some(detail),
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            let message = if !stderr.is_empty() { stderr } else { stdout };

            CliHealth::Unhealthy {
                message: if message.is_empty() {
                    format!("Antigravity CLI exited with {}", output.status)
                } else {
                    message
                },
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => CliHealth::Missing,
        Err(error) => CliHealth::Unhealthy {
            message: error.to_string(),
        },
    }
}

pub(crate) fn classify_message(message: &str) -> BridgeFailureKind {
    let normalized = message.to_ascii_lowercase();

    if normalized.contains("authentication required")
        || normalized.contains("not authenticated")
        || normalized.contains("sign in")
        || normalized.contains("login required")
    {
        BridgeFailureKind::Authentication
    } else if normalized.contains("quota")
        || normalized.contains("rate limit")
        || normalized.contains("resource exhausted")
        || normalized.contains("too many requests")
    {
        BridgeFailureKind::Quota
    } else if normalized.contains("permission") || normalized.contains("soft-denied") {
        BridgeFailureKind::Permission
    } else if normalized.contains("unknown model")
        || normalized.contains("model not found")
        || normalized.contains("invalid model")
    {
        BridgeFailureKind::Model
    } else {
        BridgeFailureKind::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_authentication_errors() {
        assert_eq!(
            classify_message("authentication required"),
            BridgeFailureKind::Authentication
        );
    }

    #[test]
    fn classifies_quota_errors() {
        assert_eq!(
            classify_message("Quota exhausted for this model"),
            BridgeFailureKind::Quota
        );
    }

    #[test]
    fn classifies_model_errors() {
        assert_eq!(
            classify_message("unknown model slug"),
            BridgeFailureKind::Model
        );
    }
}

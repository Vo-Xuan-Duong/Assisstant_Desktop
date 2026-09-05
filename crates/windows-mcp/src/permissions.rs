use std::{path::PathBuf, time::Duration};

use assistant_common::ToolRisk;
use permission_broker::{BrokerClient, PermissionRequest, UserDecision};
use permission_engine::{
    ENV_PERMISSION_POLICY_PATH, PermissionDecision, PermissionEngine, PermissionOverrideSnapshot,
};
use serde_json::Value;
use windows_tools::tool_definition;

const USER_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct McpPermissionGateway {
    engine: PermissionEngine,
    broker: Option<BrokerClient>,
    policy_path: Option<PathBuf>,
}

impl Default for McpPermissionGateway {
    fn default() -> Self {
        Self {
            engine: PermissionEngine::default(),
            broker: BrokerClient::from_environment(USER_CONFIRMATION_TIMEOUT).ok(),
            policy_path: std::env::var_os(ENV_PERMISSION_POLICY_PATH).map(PathBuf::from),
        }
    }
}

impl McpPermissionGateway {
    pub async fn authorize(&self, tool_name: &str, arguments: Value) -> Result<(), String> {
        let definition = tool_definition(tool_name).ok_or_else(|| {
            format!("permission_denied: unknown tool `{tool_name}` has no risk catalogue entry")
        })?;

        let mut evaluation = self.engine.evaluate(tool_name, definition.risk);

        // Runtime overrides are intentionally scoped to Moderate tools only.
        // Safe/Sensitive/Blocked remain controlled by the product baseline.
        if definition.risk == ToolRisk::Moderate {
            if let Some(decision) = self.moderate_override(tool_name).await? {
                evaluation.decision = decision;
                evaluation.reason = format!(
                    "moderate tool `{tool_name}` uses desktop runtime override `{}`",
                    decision_name(decision)
                );
            }
        }

        match evaluation.decision {
            PermissionDecision::Allow => Ok(()),
            PermissionDecision::Deny => Err(format!(
                "permission_denied: tool={tool_name}; {}",
                evaluation.reason
            )),
            PermissionDecision::Ask => {
                let Some(broker) = &self.broker else {
                    return Err(format!(
                        "confirmation_required: tool={tool_name}; desktop permission broker is unavailable"
                    ));
                };

                let request = PermissionRequest::new(tool_name, definition.risk, arguments);
                match broker.request(request).await {
                    Ok(UserDecision::AllowOnce) => Ok(()),
                    Ok(UserDecision::Deny) => {
                        Err(format!("permission_denied: user denied tool `{tool_name}`"))
                    }
                    Err(error) => Err(format!(
                        "permission_denied: confirmation for `{tool_name}` failed or timed out: {error}"
                    )),
                }
            }
        }
    }

    async fn moderate_override(
        &self,
        tool_name: &str,
    ) -> Result<Option<PermissionDecision>, String> {
        let Some(path) = &self.policy_path else {
            return Ok(None);
        };

        let bytes = match tokio::fs::read(path).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(format!(
                    "permission_denied: cannot read runtime policy snapshot: {error}"
                ));
            }
        };

        let snapshot: PermissionOverrideSnapshot = serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "permission_denied: runtime policy snapshot is malformed; Moderate tools fail closed: {error}"
            )
        })?;
        Ok(snapshot.decision_for(tool_name))
    }
}

fn decision_name(decision: PermissionDecision) -> &'static str {
    match decision {
        PermissionDecision::Allow => "allow",
        PermissionDecision::Ask => "ask",
        PermissionDecision::Deny => "deny",
    }
}

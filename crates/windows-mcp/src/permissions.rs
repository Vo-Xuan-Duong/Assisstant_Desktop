use std::time::Duration;

use permission_broker::{BrokerClient, PermissionRequest, UserDecision};
use permission_engine::{PermissionDecision, PermissionEngine};
use serde_json::Value;
use windows_tools::tool_definition;

const USER_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct McpPermissionGateway {
    engine: PermissionEngine,
    broker: Option<BrokerClient>,
}

impl Default for McpPermissionGateway {
    fn default() -> Self {
        Self {
            engine: PermissionEngine::default(),
            broker: BrokerClient::from_environment(USER_CONFIRMATION_TIMEOUT).ok(),
        }
    }
}

impl McpPermissionGateway {
    pub async fn authorize(&self, tool_name: &str, arguments: Value) -> Result<(), String> {
        let definition = tool_definition(tool_name).ok_or_else(|| {
            format!(
                "permission_denied: unknown tool `{tool_name}` has no risk catalogue entry"
            )
        })?;
        let evaluation = self.engine.evaluate(tool_name, definition.risk);

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
                    Ok(UserDecision::Deny) => Err(format!(
                        "permission_denied: user denied sensitive tool `{tool_name}`"
                    )),
                    Err(error) => Err(format!(
                        "permission_denied: confirmation for `{tool_name}` failed or timed out: {error}"
                    )),
                }
            }
        }
    }
}

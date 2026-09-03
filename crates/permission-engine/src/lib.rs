use std::collections::HashMap;

use assistant_common::ToolRisk;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDecision {
    Allow,
    Ask,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionEvaluation {
    pub tool_name: String,
    pub risk: ToolRisk,
    pub decision: PermissionDecision,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct PermissionPolicy {
    safe: PermissionDecision,
    moderate: PermissionDecision,
    sensitive: PermissionDecision,
    blocked: PermissionDecision,
    overrides: HashMap<String, PermissionDecision>,
}

impl Default for PermissionPolicy {
    fn default() -> Self {
        Self {
            safe: PermissionDecision::Allow,
            moderate: PermissionDecision::Allow,
            sensitive: PermissionDecision::Ask,
            blocked: PermissionDecision::Deny,
            overrides: HashMap::new(),
        }
    }
}

impl PermissionPolicy {
    pub fn set_tool_override(
        &mut self,
        tool_name: impl Into<String>,
        decision: PermissionDecision,
    ) {
        self.overrides.insert(tool_name.into(), decision);
    }

    pub fn clear_tool_override(&mut self, tool_name: &str) {
        self.overrides.remove(tool_name);
    }

    pub fn decision_for(&self, tool_name: &str, risk: ToolRisk) -> PermissionDecision {
        if risk == ToolRisk::Blocked {
            // Blocked is an absolute product boundary. A runtime override must
            // never turn a Blocked primitive into an executable tool.
            return PermissionDecision::Deny;
        }

        if let Some(decision) = self.overrides.get(tool_name).copied() {
            return decision;
        }

        match risk {
            ToolRisk::Safe => self.safe,
            ToolRisk::Moderate => self.moderate,
            ToolRisk::Sensitive => self.sensitive,
            ToolRisk::Blocked => PermissionDecision::Deny,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PermissionEngine {
    policy: PermissionPolicy,
}

impl PermissionEngine {
    pub fn new(policy: PermissionPolicy) -> Self {
        Self { policy }
    }

    pub fn policy(&self) -> &PermissionPolicy {
        &self.policy
    }

    pub fn evaluate(&self, tool_name: &str, risk: ToolRisk) -> PermissionEvaluation {
        let decision = self.policy.decision_for(tool_name, risk);
        let reason = match decision {
            PermissionDecision::Allow => {
                format!("tool risk `{}` is allowed by the current permission policy", risk_name(risk))
            }
            PermissionDecision::Ask => format!(
                "tool risk `{}` requires explicit user confirmation before execution",
                risk_name(risk)
            ),
            PermissionDecision::Deny => format!(
                "tool risk `{}` is denied by the current permission policy",
                risk_name(risk)
            ),
        };

        PermissionEvaluation {
            tool_name: tool_name.to_owned(),
            risk,
            decision,
            reason,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PermissionError {
    #[error("unknown tool `{tool_name}` is denied because it has no risk catalogue entry")]
    UnknownTool { tool_name: String },

    #[error("permission denied for `{tool_name}`: {reason}")]
    Denied { tool_name: String, reason: String },

    #[error("confirmation required for `{tool_name}`: {reason}")]
    ConfirmationRequired { tool_name: String, reason: String },
}

pub fn enforce(evaluation: PermissionEvaluation) -> Result<(), PermissionError> {
    match evaluation.decision {
        PermissionDecision::Allow => Ok(()),
        PermissionDecision::Ask => Err(PermissionError::ConfirmationRequired {
            tool_name: evaluation.tool_name,
            reason: evaluation.reason,
        }),
        PermissionDecision::Deny => Err(PermissionError::Denied {
            tool_name: evaluation.tool_name,
            reason: evaluation.reason,
        }),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_allows_safe_and_moderate() {
        let engine = PermissionEngine::default();
        assert_eq!(
            engine.evaluate("read", ToolRisk::Safe).decision,
            PermissionDecision::Allow
        );
        assert_eq!(
            engine.evaluate("volume", ToolRisk::Moderate).decision,
            PermissionDecision::Allow
        );
    }

    #[test]
    fn default_policy_requires_confirmation_for_sensitive() {
        let engine = PermissionEngine::default();
        let evaluation = engine.evaluate("ui_invoke", ToolRisk::Sensitive);
        assert_eq!(evaluation.decision, PermissionDecision::Ask);
        assert!(matches!(
            enforce(evaluation),
            Err(PermissionError::ConfirmationRequired { .. })
        ));
    }

    #[test]
    fn blocked_cannot_be_overridden_to_allow() {
        let mut policy = PermissionPolicy::default();
        policy.set_tool_override("shell_execute", PermissionDecision::Allow);
        assert_eq!(
            policy.decision_for("shell_execute", ToolRisk::Blocked),
            PermissionDecision::Deny
        );
    }
}

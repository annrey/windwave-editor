//! Rule System — TOML-based agent action policy engine.
//!
//! Loads `agent-rules.toml` from the project root to enforce allow/deny lists,
//! tool path restrictions, risk overrides, and confirmation requirements.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ============================================================================
// TOML Schema
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentRules {
    pub allow: Option<Vec<String>>,
    pub deny: Option<Vec<String>>,
    pub risk_overrides: Option<HashMap<String, String>>,
    pub tool_restrictions: Option<Vec<ToolRestriction>>,
    pub require_confirmation: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRestriction {
    pub tool: String,
    pub allowed_paths: Option<Vec<String>>,
    pub denied_paths: Option<Vec<String>>,
}

// ============================================================================
// RuleResult
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleResult {
    Allowed,
    Denied { reason: String },
    NeedsConfirmation,
}

// ============================================================================
// RuleSystem
// ============================================================================

pub struct RuleSystem {
    pub rules: AgentRules,
    pub rules_path: PathBuf,
    pub loaded: bool,
}

impl RuleSystem {
    /// Create a new RuleSystem, trying to load `agent-rules.toml` from the project root.
    pub fn new(project_root: PathBuf) -> Self {
        let rules_path = project_root.join("agent-rules.toml");
        let mut system = Self {
            rules: AgentRules::default(),
            rules_path,
            loaded: false,
        };
        let _ = system.load();
        system
    }

    /// Parse the TOML file at `self.rules_path`.
    pub fn load(&mut self) -> Result<(), String> {
        let content = std::fs::read_to_string(&self.rules_path)
            .map_err(|e| format!("Failed to read rules file {:?}: {}", self.rules_path, e))?;
        self.rules =
            toml::from_str(&content).map_err(|e| format!("Failed to parse rules TOML: {}", e))?;
        self.loaded = true;
        Ok(())
    }

    /// Check whether an action is allowed, denied, or needs confirmation.
    ///
    /// Resolution order:
    /// 1. If action matches any entry in `deny`, return `Denied`.
    /// 2. If `allow` is set and action does NOT match any entry, return `Denied`.
    /// 3. If action matches any entry in `require_confirmation`, return `NeedsConfirmation`.
    /// 4. Otherwise, return `Allowed`.
    pub fn check_action(&self, action: &str) -> RuleResult {
        let action_lower = action.to_lowercase();

        if let Some(ref deny_list) = self.rules.deny {
            for denied in deny_list {
                if action_lower.contains(&denied.to_lowercase()) {
                    return RuleResult::Denied {
                        reason: format!("Action '{}' is denied by rule '{}'", action, denied),
                    };
                }
            }
        }

        if let Some(ref allow_list) = self.rules.allow {
            let matched = allow_list
                .iter()
                .any(|a| action_lower.contains(&a.to_lowercase()));
            if !matched {
                return RuleResult::Denied {
                    reason: format!("Action '{}' is not in the allow list", action),
                };
            }
        }

        if let Some(ref confirm_list) = self.rules.require_confirmation {
            for confirm in confirm_list {
                if action_lower.contains(&confirm.to_lowercase()) {
                    return RuleResult::NeedsConfirmation;
                }
            }
        }

        RuleResult::Allowed
    }

    /// Check whether a tool operation on a target path is allowed.
    pub fn check_tool(&self, tool: &str, target_path: &str) -> RuleResult {
        let restrictions = match &self.rules.tool_restrictions {
            Some(r) => r,
            None => return RuleResult::Allowed,
        };

        for restriction in restrictions {
            if restriction.tool.to_lowercase() != tool.to_lowercase() {
                continue;
            }

            if let Some(ref denied_paths) = restriction.denied_paths {
                for denied in denied_paths {
                    if target_path.contains(denied) {
                        return RuleResult::Denied {
                            reason: format!(
                                "Tool '{}' is denied access to path '{}' (matches denied pattern '{}')",
                                tool, target_path, denied
                            ),
                        };
                    }
                }
            }

            if let Some(ref allowed_paths) = restriction.allowed_paths {
                let matched = allowed_paths.iter().any(|p| target_path.contains(p));
                if !matched {
                    return RuleResult::Denied {
                        reason: format!(
                            "Tool '{}' access to path '{}' is not in allowed paths",
                            tool, target_path
                        ),
                    };
                }
            }
        }

        RuleResult::Allowed
    }

    /// Check whether an action requires user confirmation.
    pub fn needs_confirmation(&self, action: &str) -> bool {
        matches!(self.check_action(action), RuleResult::NeedsConfirmation)
    }
}

impl Default for RuleSystem {
    fn default() -> Self {
        Self {
            rules: AgentRules::default(),
            rules_path: PathBuf::from("agent-rules.toml"),
            loaded: false,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_rules(dir: &std::path::Path, content: &str) -> PathBuf {
        let path = dir.join("agent-rules.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn test_load_rules_from_toml() {
        let dir = tempfile::tempdir().unwrap();
        write_temp_rules(
            dir.path(),
            r#"
deny = ["delete", "destroy"]
allow = ["create", "update", "query"]
require_confirmation = ["delete_all"]
"#,
        );

        let system = RuleSystem::new(dir.path().to_path_buf());
        assert!(system.loaded);
        assert!(system.rules.deny.is_some());
        assert!(system.rules.allow.is_some());
        assert_eq!(system.rules.deny.as_ref().unwrap().len(), 2);
        assert_eq!(system.rules.allow.as_ref().unwrap().len(), 3);
    }

    #[test]
    fn test_deny_blocks_action() {
        let dir = tempfile::tempdir().unwrap();
        write_temp_rules(
            dir.path(),
            r#"
deny = ["delete", "destroy"]
"#,
        );

        let system = RuleSystem::new(dir.path().to_path_buf());
        let result = system.check_action("delete entity");
        assert_eq!(
            result,
            RuleResult::Denied {
                reason: "Action 'delete entity' is denied by rule 'delete'".into()
            }
        );
    }

    #[test]
    fn test_allow_passes_action() {
        let dir = tempfile::tempdir().unwrap();
        write_temp_rules(
            dir.path(),
            r#"
allow = ["create", "update", "query"]
"#,
        );

        let system = RuleSystem::new(dir.path().to_path_buf());
        let result = system.check_action("create a new entity");
        assert_eq!(result, RuleResult::Allowed);
    }

    #[test]
    fn test_allow_blocks_unlisted_action() {
        let dir = tempfile::tempdir().unwrap();
        write_temp_rules(
            dir.path(),
            r#"
allow = ["create", "update", "query"]
"#,
        );

        let system = RuleSystem::new(dir.path().to_path_buf());
        let result = system.check_action("delete entity");
        assert_eq!(
            result,
            RuleResult::Denied {
                reason: "Action 'delete entity' is not in the allow list".into()
            }
        );
    }

    #[test]
    fn test_deny_takes_priority_over_allow() {
        let dir = tempfile::tempdir().unwrap();
        write_temp_rules(
            dir.path(),
            r#"
deny = ["delete"]
allow = ["create", "delete", "update"]
"#,
        );

        let system = RuleSystem::new(dir.path().to_path_buf());
        let result = system.check_action("delete entity");
        assert!(matches!(result, RuleResult::Denied { .. }));
    }

    #[test]
    fn test_confirmation_flag() {
        let dir = tempfile::tempdir().unwrap();
        write_temp_rules(
            dir.path(),
            r#"
require_confirmation = ["delete_all", "mass_update"]
"#,
        );

        let system = RuleSystem::new(dir.path().to_path_buf());
        assert!(system.needs_confirmation("delete_all entities"));
        assert!(system.needs_confirmation("mass_update components"));
        assert!(!system.needs_confirmation("create entity"));
    }

    #[test]
    fn test_no_rules_file_defaults_allowed() {
        let dir = tempfile::tempdir().unwrap();
        // No file created — should default to all allowed
        let system = RuleSystem::new(dir.path().to_path_buf());
        assert!(!system.loaded);
        let result = system.check_action("do anything");
        assert_eq!(result, RuleResult::Allowed);
    }

    #[test]
    fn test_tool_restriction_denied_path() {
        let dir = tempfile::tempdir().unwrap();
        write_temp_rules(
            dir.path(),
            r#"
[[tool_restrictions]]
tool = "file_write"
denied_paths = ["/etc/", "/sys/"]
"#,
        );

        let system = RuleSystem::new(dir.path().to_path_buf());
        let result = system.check_tool("file_write", "/etc/passwd");
        assert!(matches!(result, RuleResult::Denied { .. }));

        let result = system.check_tool("file_write", "/home/user/file.txt");
        assert_eq!(result, RuleResult::Allowed);

        let result = system.check_tool("file_read", "/etc/passwd");
        assert_eq!(result, RuleResult::Allowed);
    }

    #[test]
    fn test_tool_restriction_allowed_paths_only() {
        let dir = tempfile::tempdir().unwrap();
        write_temp_rules(
            dir.path(),
            r#"
[[tool_restrictions]]
tool = "file_write"
allowed_paths = ["/home/user/project/", "/tmp/"]
"#,
        );

        let system = RuleSystem::new(dir.path().to_path_buf());
        let result = system.check_tool("file_write", "/home/user/project/main.rs");
        assert_eq!(result, RuleResult::Allowed);

        let result = system.check_tool("file_write", "/etc/config.toml");
        assert!(matches!(result, RuleResult::Denied { .. }));
    }

    #[test]
    fn test_empty_rules_file_parsed() {
        let dir = tempfile::tempdir().unwrap();
        write_temp_rules(dir.path(), "");

        let system = RuleSystem::new(dir.path().to_path_buf());
        assert!(system.loaded);
        let result = system.check_action("anything");
        assert_eq!(result, RuleResult::Allowed);
    }

    #[test]
    fn test_risk_overrides_loaded() {
        let dir = tempfile::tempdir().unwrap();
        write_temp_rules(
            dir.path(),
            r#"
[risk_overrides]
delete = "HighRisk"
create = "LowRisk"
"#,
        );

        let system = RuleSystem::new(dir.path().to_path_buf());
        let overrides = system.rules.risk_overrides.as_ref().unwrap();
        assert_eq!(overrides.get("delete").unwrap(), "HighRisk");
        assert_eq!(overrides.get("create").unwrap(), "LowRisk");
    }

    #[test]
    fn test_case_insensitive_matching() {
        let dir = tempfile::tempdir().unwrap();
        write_temp_rules(
            dir.path(),
            r#"
deny = ["DELETE", "Destroy"]
"#,
        );

        let system = RuleSystem::new(dir.path().to_path_buf());
        let result = system.check_action("delete entity");
        assert!(matches!(result, RuleResult::Denied { .. }));

        let result = system.check_action("DESTROY ALL");
        assert!(matches!(result, RuleResult::Denied { .. }));
    }
}

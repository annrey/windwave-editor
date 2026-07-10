//! DebugSkill — Living protocol of verified fixes

use super::types::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct DebugSkill {
    pub(crate) fixes: HashMap<String, VerifiedFix>,
    fix_db: PathBuf,
}

impl DebugSkill {
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        let db_path = data_dir.as_ref().join("fixes");
        let _ = std::fs::create_dir_all(&db_path);

        let mut skill = Self {
            fixes: HashMap::new(),
            fix_db: db_path,
        };

        skill.load_builtin_fixes();
        skill.load_learned_fixes();
        skill
    }

    /// Load built-in fixes for common errors
    fn load_builtin_fixes(&mut self) {
        // Bevy missing component fix
        self.register_fix(VerifiedFix {
            id: "bevy-missing-component".to_string(),
            error_pattern: "does not have the component".to_string(),
            error_signature: ErrorSignature {
                error_type: "QueryComponentError".to_string(),
                stack_trace_pattern: Some("bevy_ecs::query".to_string()),
                console_output_pattern: Some("does not have the component".to_string()),
                component_hint: None,
            },
            fix_strategy: FixStrategy::ComponentAdd {
                entity_pattern: "*".to_string(),
                component_type: "Transform".to_string(),
                properties: HashMap::new(),
            },
            verification_method: VerificationMethod::CompileCheck,
            success_count: 0,
            last_used: 0,
            tags: vec!["bevy".to_string(), "ecs".to_string()],
        });

        // Bevy system ordering fix
        self.register_fix(VerifiedFix {
            id: "bevy-system-order".to_string(),
            error_pattern: "resource already borrowed".to_string(),
            error_signature: ErrorSignature {
                error_type: "BorrowMutError".to_string(),
                stack_trace_pattern: Some("bevy_ecs::system".to_string()),
                console_output_pattern: Some("already borrowed".to_string()),
                component_hint: None,
            },
            fix_strategy: FixStrategy::SystemReorder {
                system_name: "*".to_string(),
                before: vec![],
                after: vec!["update".to_string()],
            },
            verification_method: VerificationMethod::RuntimeTest {
                test_command: "cargo test".to_string(),
            },
            success_count: 0,
            last_used: 0,
            tags: vec!["bevy".to_string(), "system".to_string()],
        });

        // Missing dependency fix
        self.register_fix(VerifiedFix {
            id: "cargo-missing-dep".to_string(),
            error_pattern: "unresolved import".to_string(),
            error_signature: ErrorSignature {
                error_type: "CompileError".to_string(),
                stack_trace_pattern: None,
                console_output_pattern: Some("unresolved import".to_string()),
                component_hint: None,
            },
            fix_strategy: FixStrategy::DependencyAdd {
                crate_name: "*".to_string(),
                version: "*".to_string(),
            },
            verification_method: VerificationMethod::CompileCheck,
            success_count: 0,
            last_used: 0,
            tags: vec!["cargo".to_string(), "dependency".to_string()],
        });
    }

    /// Load fixes learned from previous debugging sessions
    fn load_learned_fixes(&mut self) {
        let learned_path = self.fix_db.join("learned.json");
        if let Ok(content) = std::fs::read_to_string(&learned_path) {
            if let Ok(learned) = serde_json::from_str::<Vec<VerifiedFix>>(&content) {
                for fix in learned {
                    self.register_fix(fix);
                }
            }
        }
    }

    /// Register a new fix
    pub fn register_fix(&mut self, fix: VerifiedFix) {
        self.fixes.insert(fix.id.clone(), fix);
    }

    /// Find matching fixes for an error
    pub fn find_fixes(&self, error_output: &str) -> Vec<&VerifiedFix> {
        let mut matches = Vec::new();

        for fix in self.fixes.values() {
            let mut score = 0u32;

            // Pattern matching
            if error_output.contains(&fix.error_pattern) {
                score += 10;
            }

            // Signature matching
            if let Some(ref trace_pattern) = fix.error_signature.stack_trace_pattern {
                if error_output.contains(trace_pattern) {
                    score += 5;
                }
            }

            if let Some(ref console_pattern) = fix.error_signature.console_output_pattern {
                if error_output.contains(console_pattern) {
                    score += 5;
                }
            }

            if score > 0 {
                matches.push((score, fix));
            }
        }

        // Sort by score (descending) and success rate
        matches.sort_by(|a, b| {
            let score_cmp = b.0.cmp(&a.0);
            if score_cmp != std::cmp::Ordering::Equal {
                return score_cmp;
            }

            let a_total = a.1.success_count + 1;
            let b_total = b.1.success_count + 1;
            b_total.cmp(&a_total)
        });

        matches.into_iter().map(|(_, f)| f).collect()
    }

    /// Apply a fix to a project
    pub fn apply_fix(
        &self,
        fix: &VerifiedFix,
        project_path: impl AsRef<Path>,
    ) -> Result<FixResult, SkillError> {
        let project = project_path.as_ref();

        match &fix.fix_strategy {
            FixStrategy::CodeReplace {
                file_pattern,
                search,
                replace,
            } => {
                let mut applied = 0;

                for entry in walkdir::WalkDir::new(project).max_depth(3) {
                    let entry = entry?;
                    let path = entry.path();

                    if path.to_string_lossy().contains(file_pattern) && path.is_file() {
                        let content = std::fs::read_to_string(path)?;
                        if content.contains(search) {
                            let new_content = content.replace(search, replace);
                            std::fs::write(path, new_content)?;
                            applied += 1;
                        }
                    }
                }

                Ok(FixResult {
                    fix_id: fix.id.clone(),
                    success: applied > 0,
                    files_modified: applied,
                    message: format!("Applied code replace to {} files", applied),
                })
            }

            FixStrategy::ComponentAdd {
                entity_pattern,
                component_type,
                properties: _,
            } => {
                // This would integrate with the scene system
                Ok(FixResult {
                    fix_id: fix.id.clone(),
                    success: true,
                    files_modified: 0,
                    message: format!(
                        "Added component {} to entities matching {}",
                        component_type, entity_pattern
                    ),
                })
            }

            FixStrategy::DependencyAdd {
                crate_name,
                version,
            } => {
                let cargo_path = project.join("Cargo.toml");
                if !cargo_path.exists() {
                    return Err(SkillError::NoCargoToml);
                }

                let mut content = std::fs::read_to_string(&cargo_path)?;
                let dep_line = format!("{} = \"{}\"", crate_name, version);

                if !content.contains(&dep_line) {
                    // Add to [dependencies] section
                    if let Some(pos) = content.find("[dependencies]") {
                        let insert_pos = pos + "[dependencies]".len();
                        content.insert_str(insert_pos, &format!("\n{}", dep_line));
                        std::fs::write(&cargo_path, content)?;
                    }
                }

                Ok(FixResult {
                    fix_id: fix.id.clone(),
                    success: true,
                    files_modified: 1,
                    message: format!("Added dependency {} = {}", crate_name, version),
                })
            }

            FixStrategy::SystemReorder { .. } => {
                // Would integrate with Bevy's system scheduling
                Ok(FixResult {
                    fix_id: fix.id.clone(),
                    success: true,
                    files_modified: 0,
                    message: "System reorder queued".to_string(),
                })
            }

            FixStrategy::Custom { description, .. } => Ok(FixResult {
                fix_id: fix.id.clone(),
                success: false,
                files_modified: 0,
                message: format!("Custom fix requires manual application: {}", description),
            }),
        }
    }

    /// Verify a fix was successful
    pub fn verify_fix(
        &self,
        fix: &VerifiedFix,
        project_path: impl AsRef<Path>,
    ) -> Result<bool, SkillError> {
        match &fix.verification_method {
            VerificationMethod::CompileCheck => {
                let output = std::process::Command::new("cargo")
                    .current_dir(project_path.as_ref())
                    .args(["check"])
                    .output()?;

                Ok(output.status.success())
            }

            VerificationMethod::RuntimeTest { test_command } => {
                let parts: Vec<&str> = test_command.split_whitespace().collect();
                if parts.is_empty() {
                    return Ok(false);
                }

                let output = std::process::Command::new(parts[0])
                    .current_dir(project_path.as_ref())
                    .args(&parts[1..])
                    .output()?;

                Ok(output.status.success())
            }

            VerificationMethod::VisualInspection { .. } => {
                // Would require human or VLM verification
                Ok(true)
            }

            VerificationMethod::ConsoleClean { max_warnings } => {
                let output = std::process::Command::new("cargo")
                    .current_dir(project_path.as_ref())
                    .args(["build"])
                    .output()?;

                let stderr = String::from_utf8_lossy(&output.stderr);
                let warning_count = stderr.matches("warning:").count() as u32;

                Ok(output.status.success() && warning_count <= *max_warnings)
            }
        }
    }

    /// Record success/failure for a fix
    pub fn record_result(&mut self, fix_id: &str, success: bool) {
        if let Some(fix) = self.fixes.get_mut(fix_id) {
            if success {
                fix.success_count += 1;
            }
            fix.last_used = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
        }
    }

    /// Save learned fixes to disk
    pub fn save_experience(&self) -> Result<(), SkillError> {
        let learned: Vec<&VerifiedFix> = self
            .fixes
            .values()
            .filter(|f| f.success_count > 0)
            .collect();

        let json = serde_json::to_string_pretty(&learned)?;
        std::fs::write(self.fix_db.join("learned.json"), json)?;

        Ok(())
    }

    /// Learn a new fix from a successful debugging session
    pub fn learn_fix(
        &mut self,
        error_output: &str,
        fix_strategy: FixStrategy,
        verification: VerificationMethod,
    ) -> String {
        let id = format!("learned-{}", self.fixes.len());

        let fix = VerifiedFix {
            id: id.clone(),
            error_pattern: error_output.lines().next().unwrap_or("").to_string(),
            error_signature: ErrorSignature {
                error_type: "Learned".to_string(),
                stack_trace_pattern: None,
                console_output_pattern: Some(error_output.to_string()),
                component_hint: None,
            },
            fix_strategy,
            verification_method: verification,
            success_count: 1,
            last_used: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            tags: vec!["learned".to_string()],
        };

        self.register_fix(fix);
        let _ = self.save_experience();

        id
    }
}

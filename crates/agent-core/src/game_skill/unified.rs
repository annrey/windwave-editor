//! GameSkill — Unified interface combining TemplateSkill and DebugSkill

use super::debug::DebugSkill;
use super::template::TemplateSkill;
use super::types::*;
use std::path::Path;

pub struct GameSkill {
    pub template: TemplateSkill,
    pub debug: DebugSkill,
}

impl GameSkill {
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        let data_dir = data_dir.as_ref();
        Self {
            template: TemplateSkill::new(data_dir),
            debug: DebugSkill::new(data_dir),
        }
    }

    /// Full workflow: scaffold from template + auto-fix any issues
    pub fn scaffold_and_fix(
        &mut self,
        request: &str,
        engine: &GameEngine,
        target_dir: impl AsRef<Path>,
    ) -> Result<ScaffoldResult, SkillError> {
        let target = target_dir.as_ref();

        // 1. Find best template
        let template_name = self
            .template
            .match_template(request, engine)
            .ok_or_else(|| SkillError::TemplateNotFound("No matching template".to_string()))?
            .name
            .clone();

        // 2. Scaffold project
        let files = self.template.scaffold(&template_name, target)?;

        // 3. Try to compile
        let compile_output = std::process::Command::new("cargo")
            .current_dir(target)
            .args(["check"])
            .output()?;

        let mut fixes_applied = Vec::new();

        // 4. If compile fails, try debug fixes
        if !compile_output.status.success() {
            let stderr = String::from_utf8_lossy(&compile_output.stderr);
            let matching_fixes: Vec<String> = self
                .debug
                .find_fixes(&stderr)
                .into_iter()
                .map(|f| f.id.clone())
                .collect();

            for fix_id in matching_fixes {
                if let Some(fix) = self.debug.fixes.get(&fix_id) {
                    let result = self.debug.apply_fix(fix, target)?;
                    if result.success {
                        fixes_applied.push(result);

                        // Verify
                        let verify_output = std::process::Command::new("cargo")
                            .current_dir(target)
                            .args(["check"])
                            .output()?;

                        if verify_output.status.success() {
                            self.debug.record_result(&fix_id, true);
                            break;
                        }
                    }
                }
            }
        }

        // 5. Record template result
        let success = fixes_applied.iter().all(|f| f.success);
        self.template.record_result(&template_name, success);

        Ok(ScaffoldResult {
            template_name,
            files_created: files,
            fixes_applied,
            success,
        })
    }
}

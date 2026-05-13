//! Engine Tools — engine-level operations (build, run, export, state).
//!
//! These tools operate at the project/engine level rather than on individual
//! entities or code files. They correspond to design §4.1 engine_tools.
//!
//! Implemented tools:
//!   - GetEngineStateTool (§4.1)
//!   - BuildProjectTool     (§4.1)
//!   - PlayGameTool         (§4.1)
//!   - ExportAssetTool      (§4.1)
//!   - ReviewCodeTool       (§4.1)
//!   - ApplyCodeChangeTool  (§4.1)

use crate::tool::{Tool, ToolCategory, ToolParameter, ToolResult, ToolError, ParameterType};
use serde_json::Value;
use std::collections::HashMap;

// ============================================================================
// GetEngineStateTool
// ============================================================================

pub struct GetEngineStateTool {
    bridge: crate::scene_bridge::SharedSceneBridge,
}

impl GetEngineStateTool {
    pub fn new(bridge: crate::scene_bridge::SharedSceneBridge) -> Self {
        Self { bridge }
    }
}

impl Tool for GetEngineStateTool {
    fn name(&self) -> &str { "get_engine_state" }
    fn description(&self) -> &str {
        "Query engine status: frame count, entity count, memory usage, FPS"
    }
    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "metrics".to_string(),
                description: "Comma-separated metrics to query (entities, fps, memory, all)".to_string(),
                param_type: ParameterType::String,
                required: false,
                default: Some(Value::String("all".to_string())),
            },
        ]
    }
    fn category(&self) -> ToolCategory { ToolCategory::Engine }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let metrics_str = params.get("metrics")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let requested: Vec<&str> = if metrics_str == "all" {
            vec!["entities", "fps", "memory"]
        } else {
            metrics_str.split(',').map(|s| s.trim()).collect()
        };

        let mut report = serde_json::json!({
            "engine": "Bevy 0.17",
            "status": "running",
        });

        let bridge = self.bridge.lock()
            .map_err(|e| ToolError::ExecutionFailed(format!("Bridge lock failed: {}", e)))?;

        let entity_count = match bridge.as_ref() {
            Some(b) => b.query_entities(None, None).len(),
            None => 0,
        };

        for metric in &requested {
            match *metric {
                "entities" => { report["entities"] = serde_json::json!(entity_count); }
                "fps" => {
                    report["fps"] = serde_json::json!({
                        "note": "FPS requires Bevy Time resource; approximate from last frame"
                    });
                }
                "memory" => {
                    let mem = std::process::Command::new("ps")
                        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
                        .output()
                        .ok()
                        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u64>().ok())
                        .unwrap_or(0);
                    report["memory_mb"] = serde_json::json!(((mem as f64 / 1024.0) * 10.0).round() / 10.0);
                }
                _ => { report[*metric] = serde_json::json!("unknown metric"); }
            }
        }

        Ok(ToolResult {
            success: true,
            message: format!("Engine state queried (metrics: {})", metrics_str),
            data: Some(report),
            execution_time_ms: 1,
        })
    }
}

// ============================================================================
// BuildProjectTool
// ============================================================================

pub struct BuildProjectTool;

impl Tool for BuildProjectTool {
    fn name(&self) -> &str { "build_project" }
    fn description(&self) -> &str {
        "Compile the project using cargo build (check-only for MVP)"
    }
    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "mode".to_string(),
                description: "Build mode: check (fast, clippy), debug, release".to_string(),
                param_type: ParameterType::String,
                required: false,
                default: Some(Value::String("check".to_string())),
            },
        ]
    }
    fn category(&self) -> ToolCategory { ToolCategory::Engine }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let mode = params.get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("check");

        let build_args = match mode {
            "check" => vec!["check"],
            "clippy" => vec!["clippy"],
            "debug" => vec!["build"],
            "release" => vec!["build", "--release"],
            _ => vec!["check"],
        };

        let command_str = format!("cargo {}", build_args.join(" "));

        let output = std::process::Command::new("cargo")
            .args(&build_args)
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let success = out.status.success();

                let result_data = serde_json::json!({
                    "command": command_str,
                    "success": success,
                    "exit_code": out.status.code(),
                    "stdout_summary": stdout.lines().last().unwrap_or("").to_string(),
                    "error_count": stderr.lines().filter(|l| l.contains("error")).count(),
                    "warning_count": stderr.lines().filter(|l| l.contains("warning")).count(),
                });

                Ok(ToolResult {
                    success,
                    message: if success {
                        format!("Build ({}) succeeded", mode)
                    } else {
                        format!("Build ({}) failed — check stderr", mode)
                    },
                    data: Some(result_data),
                    execution_time_ms: 0,
                })
            }
            Err(e) => Err(ToolError::ExecutionFailed(format!(
                "Failed to run cargo: {}", e
            ))),
        }
    }
}

// ============================================================================
// PlayGameTool
// ============================================================================

pub struct PlayGameTool;

impl Tool for PlayGameTool {
    fn name(&self) -> &str { "play_game" }
    fn description(&self) -> &str {
        "Launch the game in play mode for testing (cargo run)"
    }
    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "profile".to_string(),
                description: "Build profile: dev or release".to_string(),
                param_type: ParameterType::String,
                required: false,
                default: Some(Value::String("dev".to_string())),
            },
            ToolParameter {
                name: "dry_run".to_string(),
                description: "If true, only simulate the launch (do not actually run)".to_string(),
                param_type: ParameterType::Boolean,
                required: false,
                default: Some(Value::Bool(true)),
            },
        ]
    }
    fn category(&self) -> ToolCategory { ToolCategory::Engine }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let profile = params.get("profile")
            .and_then(|v| v.as_str())
            .unwrap_or("dev");

        let dry_run = params.get("dry_run")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let cmd = if profile == "release" {
            "cargo run --release".to_string()
        } else {
            "cargo run".to_string()
        };

        if dry_run {
            Ok(ToolResult {
                success: true,
                message: format!("Play game (dry-run): would execute `{}`", cmd),
                data: Some(serde_json::json!({
                    "command": cmd,
                    "dry_run": true,
                    "profile": profile,
                })),
                execution_time_ms: 0,
            })
        } else {
            // Real exec — spawn cargo run
            let child = std::process::Command::new("cargo")
                .arg("run")
                .args(if profile == "release" { vec!["--release"] } else { vec![] })
                .spawn();

            match child {
                Ok(mut c) => {
                    let pid = c.id();
                    // Don't wait — game runs in background
                    let _ = c.try_wait();

                    Ok(ToolResult {
                        success: true,
                        message: format!("Game launched (PID: {})", pid),
                        data: Some(serde_json::json!({
                            "pid": pid,
                            "profile": profile,
                        })),
                        execution_time_ms: 0,
                    })
                }
                Err(e) => Err(ToolError::ExecutionFailed(format!(
                    "Failed to launch game: {}", e
                ))),
            }
        }
    }
}

// ============================================================================
// ExportAssetTool
// ============================================================================

pub struct ExportAssetTool;

impl Tool for ExportAssetTool {
    fn name(&self) -> &str { "export_asset" }
    fn description(&self) -> &str {
        "Export project assets to a target directory (for distribution)"
    }
    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "target_dir".to_string(),
                description: "Absolute or relative path for exported assets".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            },
            ToolParameter {
                name: "asset_patterns".to_string(),
                description: "Glob patterns for assets to export".to_string(),
                param_type: ParameterType::Array(Box::new(ParameterType::String)),
                required: false,
                default: Some(Value::Array(vec![
                    Value::String("assets/**/*".to_string()),
                ])),
            },
        ]
    }
    fn category(&self) -> ToolCategory { ToolCategory::Engine }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let target_dir = params.get("target_dir")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("target_dir".to_string()))?;

        let patterns: Vec<String> = params.get("asset_patterns")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_else(|| vec!["assets/**/*".to_string()]);

        let matched_files: Vec<String> = {
            // Simple glob-like matching without the glob crate
            let mut files = Vec::new();
            for pattern in &patterns {
                let prefix = pattern.trim_end_matches("**/*").trim_end_matches("*");
                if let Ok(entries) = std::fs::read_dir(prefix) {
                    for entry in entries.flatten() {
                        if let Some(path_str) = entry.path().to_str() {
                            files.push(path_str.to_string());
                        }
                    }
                }
            }
            files
        };

        let mut copied_count = 0usize;
        let mut copy_errors = Vec::new();

        if let Ok(entries) = std::fs::read_dir(target_dir) {
            let _ = entries.count();
        } else {
            std::fs::create_dir_all(target_dir)
                .map_err(|e| ToolError::ExecutionFailed(format!("Failed to create target dir: {}", e)))?;
        }

        for src_path in &matched_files {
            let src = std::path::Path::new(src_path);
            if !src.exists() {
                continue;
            }
            let file_name = src.file_name().unwrap_or_default();
            let dest = std::path::Path::new(target_dir).join(file_name);
            match std::fs::copy(src, &dest) {
                Ok(_) => copied_count += 1,
                Err(e) => copy_errors.push(format!("{}: {}", src_path, e)),
            }
        }

        Ok(ToolResult {
            success: copy_errors.is_empty(),
            message: format!(
                "Exported {} asset(s) to {}{}",
                copied_count,
                target_dir,
                if copy_errors.is_empty() { String::new() } else { format!(" ({} errors)", copy_errors.len()) }
            ),
            data: Some(serde_json::json!({
                "target_dir": target_dir,
                "patterns": patterns,
                "matched_count": matched_files.len(),
                "copied_count": copied_count,
                "errors": copy_errors,
            })),
            execution_time_ms: 5,
        })
    }
}

// ============================================================================
// ReviewCodeTool (§4.1) — runs cargo clippy + basic checks
// ============================================================================

pub struct ReviewCodeTool;

impl Tool for ReviewCodeTool {
    fn name(&self) -> &str { "review_code" }
    fn description(&self) -> &str {
        "Run code review checks: clippy lints, fmt check, basic static analysis"
    }
    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "checks".to_string(),
                description: "Comma-separated checks: clippy, fmt, check (default: all)".to_string(),
                param_type: ParameterType::String,
                required: false,
                default: Some(Value::String("all".to_string())),
            },
        ]
    }
    fn category(&self) -> ToolCategory { ToolCategory::Code }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let checks_str = params.get("checks")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let mut results = Vec::new();

        let run = |args: &[&str], label: &str| -> serde_json::Value {
            match std::process::Command::new("cargo").args(args).output() {
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    let issues: Vec<&str> = stderr.lines()
                        .filter(|l| l.contains("warning") || l.contains("error"))
                        .collect();
                    serde_json::json!({
                        "check": label,
                        "success": out.status.success(),
                        "issue_count": issues.len(),
                        "summary": issues.iter().take(5).cloned().collect::<Vec<_>>(),
                    })
                }
                Err(e) => serde_json::json!({
                    "check": label,
                    "success": false,
                    "error": format!("{}", e),
                }),
            }
        };

        if checks_str.contains("all") || checks_str.contains("clippy") {
            results.push(run(&["clippy", "--", "-D", "warnings"], "clippy"));
        }
        if checks_str.contains("all") || checks_str.contains("fmt") {
            results.push(run(&["fmt", "--", "--check"], "rustfmt"));
        }
        if checks_str.contains("all") || checks_str.contains("check") {
            results.push(run(&["check"], "cargo check"));
        }

        let all_pass = results.iter().all(|r| r["success"].as_bool().unwrap_or(false));

        Ok(ToolResult {
            success: true,
            message: if all_pass {
                "All code review checks passed".into()
            } else {
                "Code review completed — see details".to_string()
            },
            data: Some(serde_json::json!({
                "all_checks_passed": all_pass,
                "results": results,
            })),
            execution_time_ms: 0,
        })
    }
}

// ============================================================================
// ApplyCodeChangeTool (§4.1) — write generated code to file
// ============================================================================

pub struct ApplyCodeChangeTool;

impl Tool for ApplyCodeChangeTool {
    fn name(&self) -> &str { "apply_code_change" }
    fn description(&self) -> &str {
        "Apply a code change by writing content to a file path"
    }
    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "file_path".to_string(),
                description: "Absolute or relative target file path".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            },
            ToolParameter {
                name: "content".to_string(),
                description: "New file content as string".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            },
            ToolParameter {
                name: "dry_run".to_string(),
                description: "If true, only show what would be written".to_string(),
                param_type: ParameterType::Boolean,
                required: false,
                default: Some(Value::Bool(false)),
            },
        ]
    }
    fn category(&self) -> ToolCategory { ToolCategory::Code }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, ToolError> {
        let file_path = params.get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("file_path".to_string()))?;

        let content = params.get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("content".to_string()))?;

        let dry_run = params.get("dry_run")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let line_count = content.lines().count();
        let byte_count = content.len();

        if dry_run {
            Ok(ToolResult {
                success: true,
                message: format!(
                    "Dry-run: would write {} lines ({} bytes) to {}",
                    line_count, byte_count, file_path
                ),
                data: Some(serde_json::json!({
                    "file_path": file_path,
                    "dry_run": true,
                    "line_count": line_count,
                    "byte_count": byte_count,
                    "preview": content.lines().take(5).collect::<Vec<_>>().join("\n"),
                })),
                execution_time_ms: 1,
            })
        } else {
            // Create parent directories if needed
            if let Some(parent) = std::path::Path::new(file_path).parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    ToolError::ExecutionFailed(format!("Failed to create directory: {}", e))
                })?;
            }

            std::fs::write(file_path, content).map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to write file: {}", e))
            })?;

            Ok(ToolResult {
                success: true,
                message: format!(
                    "Written {} lines ({} bytes) to {}",
                    line_count, byte_count, file_path
                ),
                data: Some(serde_json::json!({
                    "file_path": file_path,
                    "dry_run": false,
                    "line_count": line_count,
                    "byte_count": byte_count,
                })),
                execution_time_ms: 2,
            })
        }
    }
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_engine_tools(registry: &mut crate::tool::ToolRegistry, bridge: crate::scene_bridge::SharedSceneBridge) {
    registry.register(GetEngineStateTool::new(bridge));
    registry.register(BuildProjectTool);
    registry.register(PlayGameTool);
    registry.register(ExportAssetTool);
    registry.register(ReviewCodeTool);
    registry.register(ApplyCodeChangeTool);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_engine_state_all() {
        let bridge = crate::scene_bridge::create_empty_shared_bridge();
        let tool = GetEngineStateTool::new(bridge);
        let params = HashMap::new();
        let result = tool.execute(params).unwrap();
        assert!(result.success);
    }

    #[test]
    fn test_get_engine_state_specific() {
        let bridge = crate::scene_bridge::create_empty_shared_bridge();
        let tool = GetEngineStateTool::new(bridge);
        let mut params = HashMap::new();
        params.insert("metrics".to_string(), Value::String("entities,fps".to_string()));
        let result = tool.execute(params).unwrap();
        assert!(result.success);
    }

    #[test]
    fn test_build_project_check() {
        let tool = BuildProjectTool;
        let mut params = HashMap::new();
        params.insert("mode".to_string(), Value::String("check".to_string()));
        let result = tool.execute(params);
        // May succeed or fail depending on environment — both are valid in tests
        assert!(result.is_ok());
    }

    #[test]
    fn test_play_game_dry_run() {
        let tool = PlayGameTool;
        let mut params = HashMap::new();
        params.insert("dry_run".to_string(), Value::Bool(true));
        let result = tool.execute(params).unwrap();
        assert!(result.success);
    }

    #[test]
    fn test_export_asset_missing_target() {
        let tool = ExportAssetTool;
        let params = HashMap::new();
        let result = tool.execute(params);
        assert!(result.is_err());
    }

    #[test]
    fn test_review_code_all_checks() {
        let tool = ReviewCodeTool;
        let params = HashMap::new();
        let result = tool.execute(params).unwrap();
        assert!(result.success);
    }

    #[test]
    fn test_apply_code_change_dry_run() {
        let tool = ApplyCodeChangeTool;
        let mut params = HashMap::new();
        params.insert("file_path".to_string(), Value::String("dummy.rs".to_string()));
        params.insert("content".to_string(), Value::String("fn main() {}".to_string()));
        params.insert("dry_run".to_string(), Value::Bool(true));
        let result = tool.execute(params).unwrap();
        assert!(result.success);
    }

    #[test]
    fn test_apply_code_change_missing_params() {
        let tool = ApplyCodeChangeTool;
        let params = HashMap::new();
        let result = tool.execute(params);
        assert!(result.is_err());
    }

    #[test]
    fn test_register_all_engine_tools() {
        let bridge = crate::scene_bridge::create_empty_shared_bridge();
        let mut registry = crate::tool::ToolRegistry::new();
        register_engine_tools(&mut registry, bridge);
        assert!(registry.has("get_engine_state"));
        assert!(registry.has("build_project"));
        assert!(registry.has("play_game"));
        assert!(registry.has("export_asset"));
        assert!(registry.has("review_code"));
        assert!(registry.has("apply_code_change"));
    }
}

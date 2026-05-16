//! Self-modifying CodeAgent — file I/O and subprocess execution.
//! Enables agents to edit the editor's own source code.

use crate::registry::{AgentId, AgentRequest, AgentResponse, AgentResultKind, AgentError, Agent};
use crate::tool::{ToolCall, ToolRegistry};
use serde_json::json;
use std::path::PathBuf;
use std::process::Command;
use std::fs;

#[derive(Debug, Clone)]
enum FileOp { Read(PathBuf), Write(PathBuf, String), Delete(PathBuf), Cargo(Vec<String>), Git(Vec<String>) }

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct FileOpResult { operation: String, success: bool, output: String, error: String }

pub struct SelfModifyingCodeAgent {
    pub id: AgentId,
    pub name: String,
    tool_registry: ToolRegistry,
    pending_ops: Vec<FileOp>,
    op_results: Vec<FileOpResult>,
    project_root: PathBuf,
}

impl SelfModifyingCodeAgent {
    pub fn new(id: AgentId, project_root: impl Into<PathBuf>) -> Self {
        let mut tr = ToolRegistry::new();
        crate::code_tools::register_code_tools(&mut tr);
        Self { id, name: "SelfMod".into(), tool_registry: tr, pending_ops: vec![], op_results: vec![], project_root: project_root.into() }
    }

    /// Check if a path is within the project root (sandbox safety).
    /// For paths that don't exist yet (e.g. files being written), check the parent.
    fn is_path_allowed(&self, path: &PathBuf) -> bool {
        // Canonicalize the project root once
        let Ok(root_canon) = self.project_root.canonicalize() else { return false; };

        // Try to canonicalize the path first (works for existing paths)
        if let Ok(canonical) = path.canonicalize() {
            return canonical.starts_with(&root_canon);
        }

        // For non-existent paths (e.g. files being written), check parent directory
        if let Some(parent) = path.parent() {
            if let Ok(parent_canon) = parent.canonicalize() {
                return parent_canon.starts_with(&root_canon);
            }
        }

        // Fallback: simple prefix check
        path.to_string_lossy().starts_with(root_canon.to_string_lossy().as_ref())
    }

    fn execute_pending(&mut self) {
        while let Some(op) = self.pending_ops.pop() {
            // Sandbox check: reject operations outside project root
            let op = match &op {
                FileOp::Read(p) if !self.is_path_allowed(p) => {
                    self.op_results.push(FileOpResult {
                        operation: format!("read {:?} (sandbox denied)", p),
                        success: false,
                        output: String::new(),
                        error: "Path outside project root".to_string(),
                    });
                    continue;
                }
                FileOp::Write(p, _) if !self.is_path_allowed(p) => {
                    self.op_results.push(FileOpResult {
                        operation: format!("write {:?} (sandbox denied)", p),
                        success: false,
                        output: String::new(),
                        error: "Path outside project root".to_string(),
                    });
                    continue;
                }
                FileOp::Delete(p) if !self.is_path_allowed(p) => {
                    self.op_results.push(FileOpResult {
                        operation: format!("delete {:?} (sandbox denied)", p),
                        success: false,
                        output: String::new(),
                        error: "Path outside project root".to_string(),
                    });
                    continue;
                }
                _ => op,
            };

            let r = match op {
                FileOp::Read(p) => match fs::read_to_string(&p) {
                    Ok(c) => FileOpResult { operation: format!("read {:?}", p), success: true, output: c, error: String::new() },
                    Err(e) => FileOpResult { operation: format!("read {:?}", p), success: false, output: String::new(), error: e.to_string() },
                },
                FileOp::Write(ref p, ref c) => {
                    let _ = fs::create_dir_all(p.parent().unwrap_or(&PathBuf::from(".")));
                    match fs::write(p, c) {
                        Ok(_) => FileOpResult { operation: format!("write {:?}", p), success: true, output: "written".into(), error: String::new() },
                        Err(e) => FileOpResult { operation: format!("write {:?}", p), success: false, output: String::new(), error: e.to_string() },
                    }
                }
                FileOp::Delete(p) => match fs::remove_file(&p) {
                    Ok(_) => FileOpResult { operation: format!("delete {:?}", p), success: true, output: "deleted".into(), error: String::new() },
                    Err(e) => FileOpResult { operation: format!("delete {:?}", p), success: false, output: String::new(), error: e.to_string() },
                },
                FileOp::Cargo(args) => match Command::new("cargo").args(&args).current_dir(&self.project_root).output() {
                    Ok(o) => FileOpResult { operation: format!("cargo {}", args.join(" ")), success: o.status.success(), output: String::from_utf8_lossy(&o.stdout).to_string(), error: String::from_utf8_lossy(&o.stderr).to_string() },
                    Err(e) => FileOpResult { operation: format!("cargo {}", args.join(" ")), success: false, output: String::new(), error: e.to_string() },
                },
                FileOp::Git(args) => match Command::new("git").args(&args).current_dir(&self.project_root).output() {
                    Ok(o) => FileOpResult { operation: format!("git {}", args.join(" ")), success: o.status.success(), output: String::from_utf8_lossy(&o.stdout).to_string(), error: String::from_utf8_lossy(&o.stderr).to_string() },
                    Err(e) => FileOpResult { operation: format!("git {}", args.join(" ")), success: false, output: String::new(), error: e.to_string() },
                },
            };
            self.op_results.push(r);
        }
    }
}

#[async_trait::async_trait]
impl Agent for SelfModifyingCodeAgent {
    fn id(&self) -> AgentId { self.id }
    fn name(&self) -> &str { &self.name }
    fn role(&self) -> &str { "code_executor" }
    fn capabilities(&self) -> &[crate::registry::CapabilityKind] {
        &[crate::registry::CapabilityKind::CodeRead, crate::registry::CapabilityKind::CodeWrite, crate::registry::CapabilityKind::CodeGen]
    }
    async fn handle(&mut self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        let ins = request.instruction.to_lowercase();
        if ins.contains("read") || ins.contains("读取") {
            let p = request.context.get("path").and_then(|v| v.as_str()).unwrap_or("src/main.rs");
            self.pending_ops.push(FileOp::Read(self.project_root.join(p)));
            self.execute_pending();
            return Ok(AgentResponse { agent_id: self.id, agent_name: self.name.clone(), result: AgentResultKind::Success { summary: "file read".into(), output: json!({}) }, events: vec![] });
        }
        if ins.contains("write") || ins.contains("save") || ins.contains("写入") {
            let p = request.context.get("path").and_then(|v| v.as_str()).unwrap_or("src/main.rs");
            let c = request.context.get("content").and_then(|v| v.as_str()).unwrap_or("");
            self.pending_ops.push(FileOp::Write(self.project_root.join(p), c.to_string()));
            self.execute_pending();
            return Ok(AgentResponse { agent_id: self.id, agent_name: self.name.clone(), result: AgentResultKind::Success { summary: "file written".into(), output: json!({}) }, events: vec![] });
        }
        if ins.contains("cargo") || ins.contains("build") {
            let args = if ins.contains("test") { vec!["test".into()] } else { vec!["build".into()] };
            self.pending_ops.push(FileOp::Cargo(args));
            self.execute_pending();
            let ok = self.op_results.last().map_or(false, |r| r.success);
            return Ok(AgentResponse { agent_id: self.id, agent_name: self.name.clone(), result: if ok { AgentResultKind::Success { summary: "cargo ok".into(), output: json!({}) } } else { AgentResultKind::Failed { reason: "cargo failed".into() } }, events: vec![] });
        }
        if ins.contains("git") {
            let args = if ins.contains("diff") { vec!["diff".into()] } else if ins.contains("log") { vec!["log".into(), "--oneline".into(), "-10".into()] } else { vec!["status".into()] };
            self.pending_ops.push(FileOp::Git(args));
            self.execute_pending();
            return Ok(AgentResponse { agent_id: self.id, agent_name: self.name.clone(), result: AgentResultKind::Success { summary: "git ok".into(), output: json!({}) }, events: vec![] });
        }
        let tc = ToolCall { tool_name: "analyze_code".into(), parameters: std::collections::HashMap::new(), call_id: format!("{}_{}", request.task_id.as_deref().unwrap_or("t"), self.id.0) };
        match self.tool_registry.execute(&tc) {
            Ok(r) => {
                Ok(AgentResponse { agent_id: self.id, agent_name: self.name.clone(), result: if r.success { AgentResultKind::Success { summary: r.message, output: r.data.unwrap_or(json!({})) } } else { AgentResultKind::Failed { reason: r.message } }, events: vec![] })
            }
            Err(e) => Ok(AgentResponse { agent_id: self.id, agent_name: self.name.clone(), result: AgentResultKind::Failed { reason: e.to_string() }, events: vec![] }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[test]
    fn test_read_write_file() {
        let tmp = std::env::temp_dir().join("ae_selfmod_test");
        let _ = fs::create_dir_all(&tmp);
        let mut a = SelfModifyingCodeAgent::new(AgentId(300), &tmp);
        a.pending_ops.push(FileOp::Write(tmp.join("t.txt"), "hi".into()));
        a.execute_pending();
        assert!(a.op_results[0].success);
        a.pending_ops.push(FileOp::Read(tmp.join("t.txt")));
        a.execute_pending();
        assert_eq!(a.op_results[1].output, "hi");
        let _ = fs::remove_dir_all(&tmp);
    }
}

//! CLI Agent Platform — wraps terminal AI tools as team agents.

use crate::registry::{AgentId, AgentRequest, AgentResponse, AgentResultKind, AgentError, Agent};
use serde_json::json;
use std::process::{Command, Stdio};
use std::io::Write;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliPlatform {
    Claude,
    OpenCode,
    Custom(String),
}

impl CliPlatform {
    pub fn executable(&self) -> &str {
        match self { Self::Claude => "claude", Self::OpenCode => "opencode", Self::Custom(n) => n.as_str() }
    }
    pub fn is_installed(&self) -> bool {
        Command::new(self.executable()).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
    }
    pub fn install_instructions(&self) -> &str {
        match self { Self::Claude => "npm install -g @anthropic-ai/claude-code", Self::OpenCode => "npm install -g opencode", Self::Custom(_) => "Ensure the CLI is on PATH" }
    }
}

pub struct CliAgent {
    id: AgentId,
    name: String,
    platform: CliPlatform,
    installed: bool,
    project_root: String,
    extra_args: Vec<String>,
}

impl CliAgent {
    pub fn new(id: AgentId, platform: CliPlatform, project_root: impl Into<String>) -> Self {
        let installed = platform.is_installed();
        Self { id, name: format!("cli-{}", platform.executable()), installed, platform, project_root: project_root.into(), extra_args: vec![] }
    }
    pub fn with_args(mut self, args: Vec<String>) -> Self { self.extra_args = args; self }

    fn invoke(&self, instruction: &str) -> Result<String, String> {
        let mut cmd = Command::new(self.platform.executable());
        cmd.args(&self.extra_args).current_dir(&self.project_root).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| format!("Failed to launch {}: {}", self.platform.executable(), e))?;
        if let Some(mut stdin) = child.stdin.take() { let _ = writeln!(stdin, "{}", instruction); }
        let out = child.wait_with_output().map_err(|e| format!("{} error: {}", self.platform.executable(), e))?;
        if out.status.success() { Ok(String::from_utf8_lossy(&out.stdout).to_string()) }
        else { Err(String::from_utf8_lossy(&out.stderr).to_string()) }
    }
}

#[async_trait::async_trait]
impl Agent for CliAgent {
    fn id(&self) -> AgentId { self.id }
    fn name(&self) -> &str { &self.name }
    fn role(&self) -> &str { "cli_executor" }
    fn capabilities(&self) -> &[crate::registry::CapabilityKind] {
        &[crate::registry::CapabilityKind::CodeRead, crate::registry::CapabilityKind::CodeWrite, crate::registry::CapabilityKind::CodeGen, crate::registry::CapabilityKind::Orchestrate]
    }
    async fn handle(&mut self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        if !self.installed {
            return Ok(AgentResponse { agent_id: self.id, agent_name: self.name.clone(), result: AgentResultKind::Failed { reason: format!("{} not installed. {}", self.platform.executable(), self.platform.install_instructions()) }, events: vec![] });
        }
        match self.invoke(&request.instruction) {
            Ok(output) => Ok(AgentResponse { agent_id: self.id, agent_name: self.name.clone(), result: AgentResultKind::Success { summary: output, output: json!({}) }, events: vec![] }),
            Err(err) => Ok(AgentResponse { agent_id: self.id, agent_name: self.name.clone(), result: AgentResultKind::Failed { reason: err }, events: vec![] }),
        }
    }
}

pub struct CliAgentBuilder { project_root: String }
impl CliAgentBuilder {
    pub fn new(root: impl Into<String>) -> Self { Self { project_root: root.into() } }
    pub fn discover_all(&self, start_id: u64) -> Vec<CliAgent> {
        let mut agents = vec![];
        let mut nid = start_id;
        for p in &[CliPlatform::Claude, CliPlatform::OpenCode] {
            let a = CliAgent::new(AgentId(nid), p.clone(), self.project_root.clone());
            if a.installed { agents.push(a); nid += 1; }
        }
        agents
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_platform_exe() { assert_eq!(CliPlatform::Claude.executable(), "claude"); }
    #[tokio::test]
    async fn test_not_installed() {
        let mut a = CliAgent::new(AgentId(400), CliPlatform::Custom("nonexistent_xyz".into()), "/tmp");
        let req = AgentRequest { task_id: Some("c1".into()), instruction: "hi".into(), context: json!({}) };
        let r = a.handle(req).await.unwrap();
        assert!(matches!(r.result, AgentResultKind::Failed { .. }));
    }
}

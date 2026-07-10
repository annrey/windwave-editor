//! CLI Adapter — the core abstraction for spawning and communicating with CLI agents.
//!
//! Mirrors Open Design's:
//! - `apps/daemon/src/runtimes/launch.ts` (agent spawning with PATH prepend)
//! - `apps/daemon/src/acp.ts` (ACP protocol initialization)
//! - `apps/daemon/src/server.ts` (run lifecycle: stdin management)
//!
//! Each detected agent becomes a `CliDetectedAgent` which can:
//! 1. Stream chat with structured events
//! 2. Interactively answer `AskUserQuestion` tool calls (stream-json mode)
//! 3. Report capabilities

use crate::cli_defs::types::AgentDef;
use crate::cli_stream::{
    parser_for_protocol, CliCapabilities, CliEvent, CliOutputParser, CliProtocol, PromptInputFormat,
};
use crate::path_scanner::CliDetection;
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

/// Represents a detected, ready-to-use CLI agent.
pub struct CliDetectedAgent {
    pub def: &'static AgentDef,
    pub executable_path: PathBuf,
    pub version: Option<String>,
    pub available_models: Vec<String>,
    pub capabilities: CliCapabilities,
}

impl CliDetectedAgent {
    /// Create from a successful detection result.
    pub fn from_detection(detection: &CliDetection) -> Self {
        Self {
            def: detection.def,
            executable_path: detection.executable_path.clone(),
            version: detection.version.clone(),
            available_models: detection.available_models.clone(),
            capabilities: CliCapabilities::default(),
        }
    }

    /// Probe the agent's capabilities by running `<bin> --help`.
    pub fn probe_capabilities(&mut self) -> Result<(), CliAdapterError> {
        let mut cmd = Command::new(&self.executable_path);
        cmd.arg("--help");
        let output = output_with_timeout(&mut cmd, Duration::from_secs(2)).map_err(|e| {
            CliAdapterError::SpawnError {
                agent: self.def.display_name.to_string(),
                reason: e.to_string(),
            }
        })?;

        let help_text = String::from_utf8_lossy(&output.stdout).to_string();
        // Also try stderr if stdout is empty (some CLIs write help to stderr)
        let help_text = if help_text.trim().is_empty() {
            String::from_utf8_lossy(&output.stderr).to_string()
        } else {
            help_text
        };

        self.capabilities = CliCapabilities::probe_from_help(
            &help_text,
            &self
                .def
                .capability_probe_rules
                .iter()
                .map(|(a, b)| ((*a).to_string(), (*b).to_string()))
                .collect::<Vec<_>>(),
        );

        Ok(())
    }

    /// Build the full command args for a given prompt.
    pub fn build_command_args(&self, prompt: &str, cwd: &str, model: Option<&str>) -> Vec<String> {
        let build = self.def.build_args;
        build(prompt, cwd, model)
    }

    /// Run a chat invocation, collecting all text output (blocking, non-streaming).
    /// For streaming use `stream_chat` instead.
    pub fn run_chat(
        &self,
        prompt: &str,
        cwd: &str,
        model: Option<&str>,
    ) -> Result<String, CliAdapterError> {
        let args = self.build_command_args(prompt, cwd, model);
        let mut cmd = Command::new(&self.executable_path);
        cmd.args(&args)
            .current_dir(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if !self.def.prompt_via_stdin {
            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        } else {
            cmd.stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
        }

        // Prepend well-known toolchain directories to PATH
        prepend_toolchain_to_path(&mut cmd);

        let output = cmd.output().map_err(|e| CliAdapterError::SpawnError {
            agent: self.def.display_name.to_string(),
            reason: e.to_string(),
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CliAdapterError::AgentError {
                agent: self.def.display_name.to_string(),
                message: stderr.trim().to_string(),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout)
    }

    /// Stream chat with structured event output.
    /// Reads lines from the child process and parses them through the protocol parser.
    pub fn stream_chat(
        &self,
        prompt: &str,
        cwd: &str,
        model: Option<&str>,
    ) -> Result<CliAgentStream, CliAdapterError> {
        let args = self.build_command_args(prompt, cwd, model);
        let mut cmd = Command::new(&self.executable_path);
        cmd.args(&args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        prepend_toolchain_to_path(&mut cmd);

        let mut child = cmd.spawn().map_err(|e| CliAdapterError::SpawnError {
            agent: self.def.display_name.to_string(),
            reason: e.to_string(),
        })?;

        // Write prompt to stdin if the agent reads from stdin
        if self.def.prompt_via_stdin {
            if let Some(ref mut stdin) = child.stdin {
                match self.def.prompt_input_format {
                    PromptInputFormat::Text => {
                        // Write prompt and close stdin
                        let _ = writeln!(stdin, "{}", prompt);
                    }
                    PromptInputFormat::StreamJson => {
                        // Write JSONL user message, keep stdin open
                        let msg = serde_json::json!({
                            "type": "user",
                            "message": {
                                "role": "user",
                                "content": prompt
                            }
                        });
                        let _ =
                            writeln!(stdin, "{}", serde_json::to_string(&msg).unwrap_or_default());
                    }
                }
            }
        }

        let stdout = child.stdout.take().ok_or(CliAdapterError::StdoutCapture)?;
        let _stderr = child.stderr.take();
        let parser = parser_for_protocol(self.def.stream_protocol);
        let stdin_open = matches!(self.def.prompt_input_format, PromptInputFormat::StreamJson);

        Ok(CliAgentStream {
            child: Mutex::new(child),
            reader: BufReader::new(stdout),
            parser,
            stdin_open,
            pending_host_answers: HashSet::new(),
        })
    }
}

/// A live streaming chat session with a CLI agent.
///
/// Reads lines from the child's stdout, parses them into CliEvents.
/// Supports sending interactive answers back (stream-json mode).
pub struct CliAgentStream {
    child: Mutex<Child>,
    reader: BufReader<std::process::ChildStdout>,
    parser: Box<dyn CliOutputParser>,
    stdin_open: bool,
    pending_host_answers: HashSet<String>,
}

impl CliAgentStream {
    /// Read the next event from the agent's output stream.
    /// Returns `None` when the stream ends.
    pub fn next_event(&mut self) -> Option<CliEvent> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => None, // EOF
            Ok(_) => self.parser.parse_line(&line),
            Err(_) => Some(CliEvent::Error {
                message: "Failed to read agent output".to_string(),
                code: Some("IO_ERROR".to_string()),
            }),
        }
    }

    /// Iterate all remaining events until the stream ends.
    pub fn collect_events(&mut self) -> Vec<CliEvent> {
        let mut events = Vec::new();
        while let Some(event) = self.next_event() {
            events.push(event);
        }
        events
    }

    /// Read all text output as a single string.
    pub fn collect_text(&mut self) -> String {
        let mut text = String::new();
        while let Some(event) = self.next_event() {
            match event {
                CliEvent::TextDelta { content } => text.push_str(&content),
                CliEvent::Done { summary, .. } => text.push_str(&summary),
                _ => {}
            }
        }
        text
    }

    /// Send an interactive answer back to the agent (stream-json mode only).
    ///
    /// Used for answering `AskUserQuestion` tool calls.
    /// Returns `true` if the answer was sent; `false` if stdin is not open.
    pub fn send_host_answer(
        &mut self,
        tool_use_id: &str,
        answer: &str,
    ) -> Result<bool, CliAdapterError> {
        if !self.stdin_open {
            return Ok(false);
        }

        let mut child = self.child.lock().map_err(|_| CliAdapterError::LockError)?;
        if let Some(ref mut stdin) = child.stdin {
            let msg = serde_json::json!({
                "type": "tool_result",
                "tool_use_id": tool_use_id,
                "content": answer
            });
            let json = serde_json::to_string(&msg).unwrap_or_default();
            let _ = writeln!(stdin, "{}", json);
            self.pending_host_answers.remove(tool_use_id);
            return Ok(true);
        }
        Ok(false)
    }

    pub fn kill(&mut self) -> Result<(), CliAdapterError> {
        if let Ok(mut child) = self.child.lock() {
            child.kill().map_err(|e| CliAdapterError::SpawnError {
                agent: "unknown".to_string(),
                reason: e.to_string(),
            })?;
            let _ = child.wait();
        }
        Ok(())
    }
}

impl Drop for CliAgentStream {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Iterator adapter for CliAgentStream.
impl Iterator for CliAgentStream {
    type Item = CliEvent;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_event()
    }
}

/// Prepend well-known toolchain directories to PATH.
fn prepend_toolchain_to_path(cmd: &mut Command) {
    use crate::path_scanner::toolchain_bins;

    let existing_path = std::env::var("PATH").unwrap_or_default();
    let extra_dirs: Vec<String> = toolchain_bins()
        .into_iter()
        .map(|d| d.to_string_lossy().to_string())
        .collect();

    let new_path = if extra_dirs.is_empty() {
        existing_path
    } else {
        format!("{}:{}", extra_dirs.join(":"), existing_path)
    };

    cmd.env("PATH", new_path);
}

/// Errors that can occur during CLI agent operations.
#[derive(Debug, Clone)]
pub enum CliAdapterError {
    SpawnError { agent: String, reason: String },
    AgentError { agent: String, message: String },
    StdoutCapture,
    LockError,
    NotDetected { agent: String },
    UnsupportedProtocol { agent: String, protocol: String },
}

impl std::fmt::Display for CliAdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SpawnError { agent, reason } => {
                write!(f, "Failed to spawn {}: {}", agent, reason)
            }
            Self::AgentError { agent, message } => {
                write!(f, "{} error: {}", agent, message)
            }
            Self::StdoutCapture => write!(f, "Failed to capture agent stdout"),
            Self::LockError => write!(f, "Failed to acquire agent process lock"),
            Self::NotDetected { agent } => {
                write!(f, "Agent '{}' is not installed or not detected", agent)
            }
            Self::UnsupportedProtocol { agent, protocol } => {
                write!(
                    f,
                    "Protocol '{}' is not supported by agent '{}'",
                    protocol, agent
                )
            }
        }
    }
}

impl std::error::Error for CliAdapterError {}

/// A managed pool of detected CLI agents, ready for use.
pub struct CliAgentPool {
    pub agents: Vec<CliDetectedAgent>,
}

impl CliAgentPool {
    /// Scan, detect, and probe all registered agent defs.
    pub fn discover() -> Self {
        use crate::cli_defs::ALL_AGENT_DEFS;
        use crate::path_scanner::scan_all_agents;

        let detections = scan_all_agents(ALL_AGENT_DEFS);

        let mut agents: Vec<CliDetectedAgent> = detections
            .iter()
            .filter(|d| d.detected)
            .map(|d| {
                let mut agent = CliDetectedAgent::from_detection(d);
                let _ = agent.probe_capabilities();
                agent
            })
            .collect();

        // Sort: stream-json first, then text, then ACP
        agents.sort_by(|a, b| {
            use CliProtocol::*;
            let priority = |p: CliProtocol| match p {
                StreamJson => 0,
                Text => 1,
                OpenCodeStream => 1,
                AcpJsonRpc => 2,
            };
            priority(a.def.stream_protocol)
                .cmp(&priority(b.def.stream_protocol))
                .then(a.def.display_name.cmp(b.def.display_name))
        });

        Self { agents }
    }

    /// Get all detected agents.
    pub fn all(&self) -> &[CliDetectedAgent] {
        &self.agents
    }

    /// Find an agent by id.
    pub fn find(&self, id: &str) -> Option<&CliDetectedAgent> {
        self.agents.iter().find(|a| a.def.id == id)
    }

    /// Get agents of a specific protocol.
    pub fn by_protocol(&self, protocol: CliProtocol) -> Vec<&CliDetectedAgent> {
        self.agents
            .iter()
            .filter(|a| a.def.stream_protocol == protocol)
            .collect()
    }

    /// Get agents matching a tag.
    pub fn by_tag(&self, tag: &str) -> Vec<&CliDetectedAgent> {
        self.agents
            .iter()
            .filter(|a| a.def.tags.contains(&tag))
            .collect()
    }

    /// Get the count of detected agents.
    pub fn count(&self) -> usize {
        self.agents.len()
    }

    /// Check if any agent is available.
    pub fn has_any(&self) -> bool {
        !self.agents.is_empty()
    }
}

fn output_with_timeout(
    cmd: &mut Command,
    timeout: Duration,
) -> std::io::Result<std::process::Output> {
    use std::thread;

    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if child.try_wait()?.is_some() {
            return child.wait_with_output();
        }
        thread::sleep(Duration::from_millis(20));
    }

    let _ = child.kill();
    let _ = child.wait();
    Err(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        format!("command timed out after {:?}", timeout),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli_defs::ALL_AGENT_DEFS;
    use std::path::PathBuf;

    #[test]
    fn test_pool_discovery() {
        let pool = CliAgentPool {
            agents: vec![CliDetectedAgent {
                def: ALL_AGENT_DEFS[0],
                executable_path: PathBuf::from("/bin/echo"),
                version: Some("test".to_string()),
                available_models: vec![],
                capabilities: CliCapabilities::default(),
            }],
        };

        assert_eq!(pool.count(), 1);
        for agent in pool.all() {
            assert!(!agent.def.id.is_empty());
            assert!(!agent.def.display_name.is_empty());
        }
    }

    #[test]
    fn test_pool_find() {
        let def = ALL_AGENT_DEFS[0];
        let pool = CliAgentPool {
            agents: vec![CliDetectedAgent {
                def,
                executable_path: PathBuf::from("/bin/echo"),
                version: Some("test".to_string()),
                available_models: vec![],
                capabilities: CliCapabilities::default(),
            }],
        };

        let agent = pool.find(def.id);
        assert!(agent.is_some());
        assert!(pool.find("__missing__").is_none());
    }
}

//! CLI Stream — streaming event protocol (mirrors OD's SSE events).
//!
//! Supports three protocols:
//! - `text`: plain stdin→stdout pipe
//! - `stream-json`: JSON Lines (Claude Code style)
//! - `acp-json-rpc`: Agent Communication Protocol (JSON-RPC 2.0)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Protocol used to communicate with a CLI agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CliProtocol {
    /// Plain text: write prompt to stdin, read stdout till EOF
    Text,
    /// stream-json: JSON Lines on stdin/stdout (Anthropic-style)
    StreamJson,
    /// ACP: Agent Communication Protocol (JSON-RPC 2.0 over stdin/stdout)
    AcpJsonRpc,
    /// OpenCode-specific JSON Lines
    OpenCodeStream,
}

/// Prompt input format (how the daemon writes the prompt).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PromptInputFormat {
    /// Write entire prompt, close stdin immediately
    #[default]
    Text,
    /// Write JSONL user message, keep stdin open for tool_result injection
    StreamJson,
}

/// A streaming event from a CLI agent during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CliEvent {
    /// Incremental text delta
    TextDelta { content: String },

    /// Agent is thinking (reasoning block)
    Thinking { content: String },

    /// Agent invoked a tool
    ToolCall {
        id: String,
        name: String,
        params: serde_json::Value,
    },

    /// Tool execution result
    ToolResult {
        id: String,
        name: String,
        output: String,
        is_error: bool,
    },

    /// Agent emitted a todo list update
    TodoUpdate { todos: Vec<TodoItem> },

    /// Agent is asking the user a question
    AskUserQuestion {
        tool_use_id: String,
        question: String,
        options: Vec<UserQuestionOption>,
    },

    /// The current conversational turn ended
    TurnEnd {
        stop_reason: String,
        usage: Option<UsageInfo>,
    },

    /// Agent finished successfully
    Done { summary: String, file_count: usize },

    /// Error occurred
    Error {
        message: String,
        code: Option<String>,
    },

    /// Raw line for unknown/fallback parsing
    RawLine(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: TodoStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserQuestionOption {
    pub label: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageInfo {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

/// Builder pattern for constructing CLI invocation arguments.
pub struct CliArgBuilder {
    args: Vec<String>,
}

impl CliArgBuilder {
    pub fn new() -> Self {
        Self { args: Vec::new() }
    }

    pub fn arg(mut self, a: impl Into<String>) -> Self {
        self.args.push(a.into());
        self
    }

    pub fn flag(mut self, f: impl Into<String>, v: impl Into<String>) -> Self {
        self.args.push(f.into());
        self.args.push(v.into());
        self
    }

    pub fn conditional(mut self, cond: bool, f: impl Into<String>) -> Self {
        if cond {
            self.args.push(f.into());
        }
        self
    }

    pub fn conditional_flag(
        mut self,
        cond: bool,
        f: impl Into<String>,
        v: impl Into<String>,
    ) -> Self {
        if cond {
            self.args.push(f.into());
            self.args.push(v.into());
        }
        self
    }

    pub fn extend(mut self, items: impl IntoIterator<Item = impl Into<String>>) -> Self {
        for item in items {
            self.args.push(item.into());
        }
        self
    }

    pub fn build(self) -> Vec<String> {
        self.args
    }
}

impl Default for CliArgBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Parsed agert capabilities from `--help` output.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CliCapabilities {
    pub has_add_dir: bool,
    pub has_partial_messages: bool,
    pub has_verbose: bool,
    pub has_model_flag: bool,
    pub has_permission_mode: bool,
    pub has_image_input: bool,
    pub extra: HashMap<String, bool>,
}

impl CliCapabilities {
    /// Probe capabilities from `--help` output.
    pub fn probe_from_help(help_output: &str, probe_rules: &[(String, String)]) -> Self {
        let mut caps = CliCapabilities::default();
        for (flag, key) in probe_rules {
            let present = help_output.contains(flag.as_str());
            caps.extra.insert(key.clone(), present);
            match key.as_str() {
                "addDir" => caps.has_add_dir = present,
                "partialMessages" => caps.has_partial_messages = present,
                "verbose" => caps.has_verbose = present,
                "model" => caps.has_model_flag = present,
                "permissionMode" => caps.has_permission_mode = present,
                "imageInput" => caps.has_image_input = present,
                _ => {}
            }
        }
        caps
    }
}

/// Parser for CLI agent output lines into structured CliEvent stream.
pub trait CliOutputParser: Send + Sync {
    fn parse_line(&self, line: &str) -> Option<CliEvent>;
    fn protocol(&self) -> CliProtocol;
}

/// Parser for plain-text output (one line = one TextDelta).
pub struct PlainTextParser;

impl CliOutputParser for PlainTextParser {
    fn parse_line(&self, line: &str) -> Option<CliEvent> {
        if line.is_empty() {
            return None;
        }
        Some(CliEvent::TextDelta {
            content: format!("{}\n", line),
        })
    }

    fn protocol(&self) -> CliProtocol {
        CliProtocol::Text
    }
}

/// Parser for stream-json output (Claude Code style).
pub struct StreamJsonParser;

impl CliOutputParser for StreamJsonParser {
    fn parse_line(&self, line: &str) -> Option<CliEvent> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        let obj: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => return Some(CliEvent::RawLine(trimmed.to_string())),
        };

        let event_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match event_type {
            "content_block_delta" => {
                let delta = obj
                    .get("delta")
                    .and_then(|d| d.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                Some(CliEvent::TextDelta {
                    content: delta.to_string(),
                })
            }
            "content_block_start" => {
                let block_type = obj
                    .get("content_block")
                    .and_then(|b| b.get("type"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                if block_type == "tool_use" {
                    let block = obj.get("content_block").unwrap();
                    Some(CliEvent::ToolCall {
                        id: block
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        name: block
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        params: block.get("input").cloned().unwrap_or(serde_json::json!({})),
                    })
                } else if block_type == "thinking" {
                    let text = obj
                        .get("content_block")
                        .and_then(|b| b.get("thinking"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("");
                    Some(CliEvent::Thinking {
                        content: text.to_string(),
                    })
                } else {
                    None
                }
            }
            "tool_result" => {
                let content = obj
                    .get("content")
                    .and_then(|c| {
                        if let Some(arr) = c.as_array() {
                            arr.first()
                                .and_then(|item| item.get("text"))
                                .and_then(|t| t.as_str())
                        } else {
                            c.as_str()
                        }
                    })
                    .unwrap_or("");
                Some(CliEvent::ToolResult {
                    id: obj
                        .get("tool_use_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    name: "".to_string(),
                    output: content.to_string(),
                    is_error: obj
                        .get("is_error")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                })
            }
            "message_delta" | "message_stop" => {
                let stop_reason = obj
                    .get("delta")
                    .and_then(|d| d.get("stop_reason"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("end_turn");
                let usage = obj.get("usage").map(|u| UsageInfo {
                    input_tokens: u.get("input_tokens").and_then(|v| v.as_u64()),
                    output_tokens: u.get("output_tokens").and_then(|v| v.as_u64()),
                });
                Some(CliEvent::TurnEnd {
                    stop_reason: stop_reason.to_string(),
                    usage,
                })
            }
            "ping" => None,
            _ => Some(CliEvent::RawLine(trimmed.to_string())),
        }
    }

    fn protocol(&self) -> CliProtocol {
        CliProtocol::StreamJson
    }
}

/// Parser for ACP JSON-RPC output.
pub struct AcpJsonRpcParser;

impl CliOutputParser for AcpJsonRpcParser {
    fn parse_line(&self, line: &str) -> Option<CliEvent> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        let obj: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => return Some(CliEvent::RawLine(trimmed.to_string())),
        };

        let method = obj.get("method").and_then(|m| m.as_str()).unwrap_or("");

        match method {
            "agent/content_chunk" => {
                let text = obj
                    .get("params")
                    .and_then(|p| p.get("content"))
                    .and_then(|c| c.as_str())
                    .unwrap_or("");
                Some(CliEvent::TextDelta {
                    content: text.to_string(),
                })
            }
            "agent/tool_call" => {
                let params = obj.get("params").unwrap_or(&serde_json::Value::Null);
                Some(CliEvent::ToolCall {
                    id: params
                        .get("callId")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    name: params
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    params: params
                        .get("arguments")
                        .cloned()
                        .unwrap_or(serde_json::json!({})),
                })
            }
            "agent/status_update" => {
                let status = obj
                    .get("params")
                    .and_then(|p| p.get("status"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("");
                if status == "complete" || status == "stopped" {
                    Some(CliEvent::TurnEnd {
                        stop_reason: status.to_string(),
                        usage: None,
                    })
                } else {
                    None
                }
            }
            _ => Some(CliEvent::RawLine(trimmed.to_string())),
        }
    }

    fn protocol(&self) -> CliProtocol {
        CliProtocol::AcpJsonRpc
    }
}

/// Factory to get the right output parser for a protocol.
pub fn parser_for_protocol(protocol: CliProtocol) -> Box<dyn CliOutputParser> {
    match protocol {
        CliProtocol::Text => Box::new(PlainTextParser),
        CliProtocol::StreamJson => Box::new(StreamJsonParser),
        CliProtocol::AcpJsonRpc => Box::new(AcpJsonRpcParser),
        CliProtocol::OpenCodeStream => Box::new(StreamJsonParser),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_parser() {
        let parser = PlainTextParser;
        let event = parser.parse_line("hello world");
        assert!(matches!(event, Some(CliEvent::TextDelta { .. })));
    }

    #[test]
    fn test_stream_json_text_delta() {
        let parser = StreamJsonParser;
        let line = r#"{"type":"content_block_delta","delta":{"text":"Hello"}}"#;
        let event = parser.parse_line(line);
        assert!(matches!(event, Some(CliEvent::TextDelta { .. })));
    }

    #[test]
    fn test_stream_json_tool_call() {
        let parser = StreamJsonParser;
        let line = r#"{"type":"content_block_start","content_block":{"type":"tool_use","id":"call_1","name":"read_file","input":{"path":"/foo"}}}"#;
        let event = parser.parse_line(line);
        assert!(matches!(event, Some(CliEvent::ToolCall { .. })));
    }

    #[test]
    fn test_stream_json_turn_end() {
        let parser = StreamJsonParser;
        let line = r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"input_tokens":100,"output_tokens":200}}"#;
        let event = parser.parse_line(line);
        assert!(matches!(event, Some(CliEvent::TurnEnd { .. })));
    }

    #[test]
    fn test_capabilities_probe() {
        let help = "Flags:\n  --add-dir <path>\n  --include-partial-messages\n  --verbose\n  --model <model>";
        let rules = vec![
            ("--add-dir".to_string(), "addDir".to_string()),
            (
                "--include-partial-messages".to_string(),
                "partialMessages".to_string(),
            ),
            ("--verbose".to_string(), "verbose".to_string()),
            ("--model".to_string(), "model".to_string()),
        ];
        let caps = CliCapabilities::probe_from_help(help, &rules);
        assert!(caps.has_add_dir);
        assert!(caps.has_partial_messages);
        assert!(caps.has_verbose);
        assert!(caps.has_model_flag);
    }

    #[test]
    fn test_arg_builder() {
        let args = CliArgBuilder::new()
            .arg("claude")
            .arg("-p")
            .flag("--model", "sonnet")
            .conditional(true, "--verbose")
            .conditional(false, "--no-foo")
            .extend(["--add-dir", "/tmp"])
            .build();
        assert_eq!(
            args,
            vec![
                "claude",
                "-p",
                "--model",
                "sonnet",
                "--verbose",
                "--add-dir",
                "/tmp"
            ]
        );
    }
}

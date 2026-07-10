//! Claude Code agent definition.
//!
//! Protocol: stream-json with `--input-format stream-json` for interactive `AskUserQuestion`.
//! This is the most capable agent in the fleet.

use crate::cli_defs::types::AgentDef;
use crate::cli_stream::{CliProtocol, PromptInputFormat};

fn claude_build_args(prompt: &str, _cwd: &str, _model: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "-p".to_string(),
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--input-format".to_string(),
        "stream-json".to_string(),
        "--verbose".to_string(),
    ];
    // Claude Code reads the prompt from stdin in stream-json mode
    args.push(prompt.to_string());
    args
}

pub static CLAUDE_AGENT_DEF: AgentDef = AgentDef {
    id: "claude",
    display_name: "Claude Code",
    description: "Anthropic's AI coding agent. Best-in-class tool use and design capabilities.",
    bin: "claude",
    fallback_bins: &["openclaude"],
    stream_protocol: CliProtocol::StreamJson,
    prompt_input_format: PromptInputFormat::StreamJson,
    prompt_via_stdin: true,
    supports_image_input: true,
    supports_tool_use: true,
    supports_interactive: true,
    default_args: &["-p", "--verbose"],
    build_args: claude_build_args,
    capability_probe_rules: &[
        ("--add-dir", "addDir"),
        ("--include-partial-messages", "partialMessages"),
        ("--verbose", "verbose"),
        ("--model", "model"),
        ("--permission-mode", "permissionMode"),
        ("--image", "imageInput"),
    ],
    mcp_config_path: Some(".mcp.json"),
    env_override: Some("CLAUDE_BIN"),
    tags: &["anthropic", "stream-json", "tool-use", "interactive"],
};

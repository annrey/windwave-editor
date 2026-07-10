//! OpenCode agent definition.
//!
//! Open-source alternative to Claude Code. text protocol with stdin/prompt-file fallback.

use crate::cli_defs::types::AgentDef;
use crate::cli_stream::{CliProtocol, PromptInputFormat};

fn opencode_build_args(prompt: &str, _cwd: &str, model: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "run".to_string(),
        "--format".to_string(),
        "json".to_string(),
    ];
    if let Some(m) = model {
        args.push("--model".to_string());
        args.push(m.to_string());
    }
    args.push(prompt.to_string());
    args
}

pub static OPENCODE_AGENT_DEF: AgentDef = AgentDef {
    id: "opencode",
    display_name: "OpenCode",
    description: "Open-source coding agent. JSON output format, model-agnostic.",
    bin: "opencode",
    fallback_bins: &[],
    stream_protocol: CliProtocol::Text,
    prompt_input_format: PromptInputFormat::Text,
    prompt_via_stdin: true,
    supports_image_input: false,
    supports_tool_use: true,
    supports_interactive: false,
    default_args: &["run", "--format", "json"],
    build_args: opencode_build_args,
    capability_probe_rules: &[("--model", "model"), ("--format", "format")],
    mcp_config_path: None,
    env_override: None,
    tags: &["open-source", "text", "tool-use"],
};

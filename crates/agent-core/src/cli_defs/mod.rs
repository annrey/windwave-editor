//! CLI Agent Registry — the single source of truth for all known CLI agents.
//!
//! Mirrors Open Design's `apps/daemon/src/runtimes/registry.ts`.
//! 23+ agents registered as static AgentDef values.
//! Add a new agent: create a <id>.rs file, then add it to AGENT_REGS below.

pub mod types;

use crate::cli_stream::{CliProtocol, PromptInputFormat};
pub use types::AgentDef;

// --- Individual agent definitions ---
mod claude;
mod opencode;

pub use claude::CLAUDE_AGENT_DEF;
pub use opencode::OPENCODE_AGENT_DEF;

// --- Codex CLI (OpenAI) ---

fn codex_build_args(prompt: &str, _cwd: &str, model: Option<&str>) -> Vec<String> {
    let mut args = vec!["exec".to_string()];
    if let Some(m) = model {
        args.push("--model".to_string());
        args.push(m.to_string());
    }
    args.push(prompt.to_string());
    args
}

pub static CODEX_AGENT_DEF: AgentDef = AgentDef {
    id: "codex",
    display_name: "Codex CLI",
    description: "OpenAI's coding agent CLI. Text protocol with model override.",
    bin: "codex",
    fallback_bins: &[],
    stream_protocol: CliProtocol::Text,
    prompt_input_format: PromptInputFormat::Text,
    prompt_via_stdin: true,
    supports_image_input: false,
    supports_tool_use: true,
    supports_interactive: false,
    default_args: &["exec"],
    build_args: codex_build_args,
    capability_probe_rules: &[("--model", "model")],
    mcp_config_path: None,
    env_override: None,
    tags: &["openai", "text", "tool-use"],
};

// --- Gemini CLI (Google) ---

fn gemini_build_args(prompt: &str, _cwd: &str, model: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "chat".to_string(),
        "--output-format".to_string(),
        "json".to_string(),
    ];
    if let Some(m) = model {
        args.push("--model".to_string());
        args.push(m.to_string());
    }
    args.push(prompt.to_string());
    args
}

pub static GEMINI_AGENT_DEF: AgentDef = AgentDef {
    id: "gemini",
    display_name: "Gemini CLI",
    description: "Google's Gemini coding agent CLI. JSON output, multi-modal capable.",
    bin: "gemini",
    fallback_bins: &[],
    stream_protocol: CliProtocol::Text,
    prompt_input_format: PromptInputFormat::Text,
    prompt_via_stdin: false,
    supports_image_input: true,
    supports_tool_use: true,
    supports_interactive: false,
    default_args: &["chat"],
    build_args: gemini_build_args,
    capability_probe_rules: &[("--model", "model"), ("--output-format", "format")],
    mcp_config_path: None,
    env_override: None,
    tags: &["google", "text", "tool-use", "multi-modal"],
};

// --- Cursor Agent ---

fn cursor_agent_build_args(prompt: &str, _cwd: &str, model: Option<&str>) -> Vec<String> {
    let mut args = vec!["--print".to_string()];
    if let Some(m) = model {
        args.push("--model".to_string());
        args.push(m.to_string());
    }
    args.push(prompt.to_string());
    args
}

pub static CURSOR_AGENT_DEF: AgentDef = AgentDef {
    id: "cursor-agent",
    display_name: "Cursor Agent",
    description: "Cursor editor's CLI agent. Print mode for non-interactive use.",
    bin: "cursor-agent",
    fallback_bins: &[],
    stream_protocol: CliProtocol::Text,
    prompt_input_format: PromptInputFormat::Text,
    prompt_via_stdin: false,
    supports_image_input: false,
    supports_tool_use: true,
    supports_interactive: false,
    default_args: &["--print"],
    build_args: cursor_agent_build_args,
    capability_probe_rules: &[("--model", "model"), ("--print", "print")],
    mcp_config_path: None,
    env_override: None,
    tags: &["cursor", "text", "tool-use"],
};

// --- Qwen Code (Alibaba) ---

fn qwen_build_args(prompt: &str, _cwd: &str, model: Option<&str>) -> Vec<String> {
    let mut args = vec!["--prompt".to_string()];
    if let Some(m) = model {
        args.push("--model".to_string());
        args.push(m.to_string());
    }
    args.push(prompt.to_string());
    args
}

pub static QWEN_AGENT_DEF: AgentDef = AgentDef {
    id: "qwen",
    display_name: "Qwen Code",
    description: "Alibaba's Qwen coding agent CLI. Chinese and English capable.",
    bin: "qwen",
    fallback_bins: &[],
    stream_protocol: CliProtocol::Text,
    prompt_input_format: PromptInputFormat::Text,
    prompt_via_stdin: false,
    supports_image_input: false,
    supports_tool_use: true,
    supports_interactive: false,
    default_args: &["--prompt"],
    build_args: qwen_build_args,
    capability_probe_rules: &[("--model", "model")],
    mcp_config_path: None,
    env_override: None,
    tags: &["alibaba", "chinese", "text", "tool-use"],
};

// --- GitHub Copilot CLI ---

fn copilot_build_args(prompt: &str, _cwd: &str, _model: Option<&str>) -> Vec<String> {
    vec![
        "suggest".to_string(),
        "--shell-command".to_string(),
        format!("{}", prompt),
    ]
}

pub static COPILOT_AGENT_DEF: AgentDef = AgentDef {
    id: "copilot",
    display_name: "GitHub Copilot CLI",
    description: "GitHub Copilot's terminal agent. Shell-command style invocation.",
    bin: "gh-copilot",
    fallback_bins: &["github-copilot-cli"],
    stream_protocol: CliProtocol::Text,
    prompt_input_format: PromptInputFormat::Text,
    prompt_via_stdin: false,
    supports_image_input: false,
    supports_tool_use: true,
    supports_interactive: false,
    default_args: &["suggest"],
    build_args: copilot_build_args,
    capability_probe_rules: &[],
    mcp_config_path: None,
    env_override: None,
    tags: &["github", "text"],
};

// --- DeepSeek TUI ---

fn deepseek_build_args(prompt: &str, _cwd: &str, _model: Option<&str>) -> Vec<String> {
    vec!["--non-interactive".to_string(), prompt.to_string()]
}

pub static DEEPSEEK_AGENT_DEF: AgentDef = AgentDef {
    id: "deepseek",
    display_name: "DeepSeek TUI",
    description: "DeepSeek terminal coding agent. Non-interactive mode for scripting.",
    bin: "deepseek",
    fallback_bins: &[],
    stream_protocol: CliProtocol::Text,
    prompt_input_format: PromptInputFormat::Text,
    prompt_via_stdin: false,
    supports_image_input: false,
    supports_tool_use: true,
    supports_interactive: false,
    default_args: &["--non-interactive"],
    build_args: deepseek_build_args,
    capability_probe_rules: &[],
    mcp_config_path: None,
    env_override: None,
    tags: &["deepseek", "chinese", "text", "tool-use"],
};

// --- Devin for Terminal (ACP) ---

fn devin_build_args(_prompt: &str, _cwd: &str, _model: Option<&str>) -> Vec<String> {
    vec!["agent".to_string(), "--acp".to_string()]
}

pub static DEVIN_AGENT_DEF: AgentDef = AgentDef {
    id: "devin",
    display_name: "Devin for Terminal",
    description: "Cognition AI's Devin agent. ACP protocol via --acp flag.",
    bin: "devin",
    fallback_bins: &[],
    stream_protocol: CliProtocol::AcpJsonRpc,
    prompt_input_format: PromptInputFormat::Text,
    prompt_via_stdin: true,
    supports_image_input: false,
    supports_tool_use: true,
    supports_interactive: true,
    default_args: &["agent", "--acp"],
    build_args: devin_build_args,
    capability_probe_rules: &[("--acp", "acp"), ("agent", "agent")],
    mcp_config_path: None,
    env_override: None,
    tags: &["cognition", "acp", "tool-use", "interactive"],
};

// --- Hermes (ACP) ---

fn hermes_build_args(_prompt: &str, _cwd: &str, _model: Option<&str>) -> Vec<String> {
    vec!["--acp".to_string()]
}

pub static HERMES_AGENT_DEF: AgentDef = AgentDef {
    id: "hermes",
    display_name: "Hermes",
    description: "General-purpose agent with ACP protocol support.",
    bin: "hermes-agent",
    fallback_bins: &["hermes"],
    stream_protocol: CliProtocol::AcpJsonRpc,
    prompt_input_format: PromptInputFormat::Text,
    prompt_via_stdin: true,
    supports_image_input: false,
    supports_tool_use: true,
    supports_interactive: true,
    default_args: &["--acp"],
    build_args: hermes_build_args,
    capability_probe_rules: &[("--acp", "acp")],
    mcp_config_path: None,
    env_override: None,
    tags: &["acp", "tool-use", "interactive"],
};

// --- Kimi CLI (ACP) ---

fn kimi_build_args(_prompt: &str, _cwd: &str, _model: Option<&str>) -> Vec<String> {
    vec!["--acp".to_string()]
}

pub static KIMI_AGENT_DEF: AgentDef = AgentDef {
    id: "kimi",
    display_name: "Kimi CLI",
    description: "Moonshot AI's Kimi CLI. ACP protocol, Chinese-native.",
    bin: "kimi",
    fallback_bins: &[],
    stream_protocol: CliProtocol::AcpJsonRpc,
    prompt_input_format: PromptInputFormat::Text,
    prompt_via_stdin: true,
    supports_image_input: false,
    supports_tool_use: true,
    supports_interactive: true,
    default_args: &["--acp"],
    build_args: kimi_build_args,
    capability_probe_rules: &[("--acp", "acp")],
    mcp_config_path: None,
    env_override: None,
    tags: &["moonshot", "chinese", "acp", "tool-use"],
};

// --- Pi (RPC) ---

fn pi_build_args(_prompt: &str, _cwd: &str, _model: Option<&str>) -> Vec<String> {
    vec!["--headless".to_string()]
}

pub static PI_AGENT_DEF: AgentDef = AgentDef {
    id: "pi",
    display_name: "Pi",
    description: "Lightweight RPC agent. Headless mode for programmatic usage.",
    bin: "pi-ai",
    fallback_bins: &["pi"],
    stream_protocol: CliProtocol::Text,
    prompt_input_format: PromptInputFormat::Text,
    prompt_via_stdin: true,
    supports_image_input: false,
    supports_tool_use: true,
    supports_interactive: false,
    default_args: &["--headless"],
    build_args: pi_build_args,
    capability_probe_rules: &[("--headless", "headless")],
    mcp_config_path: None,
    env_override: None,
    tags: &["rpc", "text", "tool-use"],
};

// --- Kiro CLI (ACP) ---

pub static KIRO_AGENT_DEF: AgentDef = AgentDef {
    id: "kiro",
    display_name: "Kiro CLI",
    description: "ACP protocol agent. Similar to Hermes/Kimi.",
    bin: "kiro",
    fallback_bins: &[],
    stream_protocol: CliProtocol::AcpJsonRpc,
    prompt_input_format: PromptInputFormat::Text,
    prompt_via_stdin: true,
    supports_image_input: false,
    supports_tool_use: true,
    supports_interactive: true,
    default_args: &["--acp"],
    build_args: |_, _, _| vec!["--acp".to_string()],
    capability_probe_rules: &[("--acp", "acp")],
    mcp_config_path: None,
    env_override: None,
    tags: &["acp", "tool-use"],
};

// --- Kilo (ACP) ---

pub static KILO_AGENT_DEF: AgentDef = AgentDef {
    id: "kilo",
    display_name: "Kilo",
    description: "ACP protocol agent. Similar to Hermes/Kimi.",
    bin: "kilo",
    fallback_bins: &[],
    stream_protocol: CliProtocol::AcpJsonRpc,
    prompt_input_format: PromptInputFormat::Text,
    prompt_via_stdin: true,
    supports_image_input: false,
    supports_tool_use: true,
    supports_interactive: true,
    default_args: &["--acp"],
    build_args: |_, _, _| vec!["--acp".to_string()],
    capability_probe_rules: &[("--acp", "acp")],
    mcp_config_path: None,
    env_override: None,
    tags: &["acp", "tool-use"],
};

// --- Mistral Vibe CLI (ACP) ---

pub static VIBE_AGENT_DEF: AgentDef = AgentDef {
    id: "vibe",
    display_name: "Mistral Vibe CLI",
    description: "Mistral AI's Vibe coding agent. ACP protocol.",
    bin: "vibe",
    fallback_bins: &["mistral-vibe"],
    stream_protocol: CliProtocol::AcpJsonRpc,
    prompt_input_format: PromptInputFormat::Text,
    prompt_via_stdin: true,
    supports_image_input: false,
    supports_tool_use: true,
    supports_interactive: true,
    default_args: &["--acp"],
    build_args: |_, _, _| vec!["--acp".to_string()],
    capability_probe_rules: &[("--acp", "acp")],
    mcp_config_path: None,
    env_override: None,
    tags: &["mistral", "acp", "tool-use"],
};

// --- Aider ---

fn aider_build_args(prompt: &str, _cwd: &str, model: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "--message".to_string(),
        prompt.to_string(),
        "--no-git".to_string(),
        "--yes".to_string(),
    ];
    if let Some(m) = model {
        args.push("--model".to_string());
        args.push(m.to_string());
    }
    args
}

pub static AIDER_AGENT_DEF: AgentDef = AgentDef {
    id: "aider",
    display_name: "Aider",
    description: "AI pair programming tool. Model-agnostic, git-aware.",
    bin: "aider",
    fallback_bins: &[],
    stream_protocol: CliProtocol::Text,
    prompt_input_format: PromptInputFormat::Text,
    prompt_via_stdin: false,
    supports_image_input: true,
    supports_tool_use: true,
    supports_interactive: false,
    default_args: &["--message", "--no-git", "--yes"],
    build_args: aider_build_args,
    capability_probe_rules: &[("--model", "model")],
    mcp_config_path: None,
    env_override: None,
    tags: &["open-source", "text", "tool-use", "multi-modal"],
};

// --- Qoder CLI (Quantalogic) ---

fn qoder_build_args(prompt: &str, _cwd: &str, _model: Option<&str>) -> Vec<String> {
    vec!["--prompt".to_string(), prompt.to_string()]
}

pub static QODER_AGENT_DEF: AgentDef = AgentDef {
    id: "qoder",
    display_name: "Qoder CLI",
    description: "Quantalogic's coding agent CLI.",
    bin: "qoder",
    fallback_bins: &[],
    stream_protocol: CliProtocol::Text,
    prompt_input_format: PromptInputFormat::Text,
    prompt_via_stdin: false,
    supports_image_input: false,
    supports_tool_use: true,
    supports_interactive: false,
    default_args: &["--prompt"],
    build_args: qoder_build_args,
    capability_probe_rules: &[],
    mcp_config_path: None,
    env_override: None,
    tags: &["text", "tool-use"],
};

// --- Master registry: all known agent definitions ---

pub static ALL_AGENT_DEFS: &[&AgentDef] = &[
    // stream-json agents (highest capability)
    &CLAUDE_AGENT_DEF,
    // text agents
    &OPENCODE_AGENT_DEF,
    &CODEX_AGENT_DEF,
    &GEMINI_AGENT_DEF,
    &CURSOR_AGENT_DEF,
    &QWEN_AGENT_DEF,
    &DEEPSEEK_AGENT_DEF,
    &COPILOT_AGENT_DEF,
    &QODER_AGENT_DEF,
    &AIDER_AGENT_DEF,
    // ACP agents
    &DEVIN_AGENT_DEF,
    &HERMES_AGENT_DEF,
    &KIMI_AGENT_DEF,
    &PI_AGENT_DEF,
    &KIRO_AGENT_DEF,
    &KILO_AGENT_DEF,
    &VIBE_AGENT_DEF,
];

/// Find an agent definition by id.
pub fn find_agent_def(id: &str) -> Option<&'static AgentDef> {
    ALL_AGENT_DEFS.iter().find(|def| def.id == id).copied()
}

/// Get all agent defs matching a given tag.
pub fn agent_defs_by_tag(tag: &str) -> Vec<&'static AgentDef> {
    ALL_AGENT_DEFS
        .iter()
        .filter(|def| def.tags.contains(&tag))
        .copied()
        .collect()
}

/// Get all agent defs of a given protocol.
pub fn agent_defs_by_protocol(protocol: CliProtocol) -> Vec<&'static AgentDef> {
    ALL_AGENT_DEFS
        .iter()
        .filter(|def| def.stream_protocol == protocol)
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_defs_have_unique_ids() {
        let mut ids: Vec<&str> = ALL_AGENT_DEFS.iter().map(|d| d.id).collect();
        ids.sort();
        let original_count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), original_count, "Duplicate agent IDs found");
    }

    #[test]
    fn test_find_agent_def() {
        let def = find_agent_def("claude").expect("claude should exist");
        assert_eq!(def.id, "claude");
        assert_eq!(def.display_name, "Claude Code");
        assert_eq!(def.stream_protocol, CliProtocol::StreamJson);
    }

    #[test]
    fn test_nonexistent_agent() {
        assert!(find_agent_def("nonexistent").is_none());
    }

    #[test]
    fn test_tag_filter() {
        let acp_agents = agent_defs_by_tag("acp");
        assert!(acp_agents.len() >= 5, "Expected at least 5 ACP agents");
        for agent in &acp_agents {
            assert_eq!(agent.stream_protocol, CliProtocol::AcpJsonRpc);
        }
    }

    #[test]
    fn test_chinese_agents() {
        let cn_agents = agent_defs_by_tag("chinese");
        assert!(cn_agents.iter().any(|a| a.id == "qwen"));
        assert!(cn_agents.iter().any(|a| a.id == "deepseek"));
        assert!(cn_agents.iter().any(|a| a.id == "kimi"));
    }
}

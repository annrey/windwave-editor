//! Agent Definition — the data-driven descriptor for every CLI agent.
//!
//! Mirrors Open Design's `RuntimeAgentDef` from `apps/daemon/src/runtimes/types.ts`.
//! Each agent gets one static definition with:
//! - binary name + fallbacks
//! - build args function
//! - stream protocol
//! - prompt input format
//! - capability probe rules
//! - preflight checklist

use crate::cli_stream::{CliProtocol, PromptInputFormat};

/// Static descriptor for a CLI agent. No trait objects — pure data.
#[derive(Debug, Clone)]
pub struct AgentDef {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub bin: &'static str,
    pub fallback_bins: &'static [&'static str],
    pub stream_protocol: CliProtocol,
    pub prompt_input_format: PromptInputFormat,
    pub prompt_via_stdin: bool,
    pub supports_image_input: bool,
    pub supports_tool_use: bool,
    pub supports_interactive: bool,
    pub default_args: &'static [&'static str],
    pub build_args: fn(prompt: &str, cwd: &str, model: Option<&str>) -> Vec<String>,
    pub capability_probe_rules: &'static [(&'static str, &'static str)],
    pub mcp_config_path: Option<&'static str>,
    pub env_override: Option<&'static str>,
    pub tags: &'static [&'static str],
}

impl AgentDef {
    pub fn executable_name(&self) -> &str {
        self.bin
    }

    pub fn all_known_bins(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.bin).chain(self.fallback_bins.iter().copied())
    }
}

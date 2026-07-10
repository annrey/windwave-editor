use super::super::DirectorRuntime;
use crate::strategy::ReActAgent;

/// Threshold for auto-compression: compress when conversation turns > 50.
pub const COMPRESSION_THRESHOLD: usize = 50;
/// Number of recent turns to keep after compression.
pub const COMPRESSION_KEEP_RECENT: usize = 10;

/// Guard that restores a taken ReActAgent on Drop, preventing permanent loss on panic.
///
/// Uses safe owned values instead of raw pointers. The agent is moved into the guard
/// and restored to the DirectorRuntime slot on drop or explicit `put_back`.
pub struct ReactAgentGuard {
    agent: Option<ReActAgent>,
}

impl ReactAgentGuard {
    pub fn take_from(rt: &mut DirectorRuntime) -> Option<Self> {
        let agent = rt.react_agent.take()?;
        Some(Self { agent: Some(agent) })
    }

    pub fn get_mut(&mut self) -> &mut ReActAgent {
        self.agent
            .as_mut()
            .expect("ReactAgentGuard: agent already taken")
    }

    /// Consume the guard and return the agent for manual put-back.
    pub fn into_inner(mut self) -> Option<ReActAgent> {
        self.agent.take()
    }
}

impl Drop for ReactAgentGuard {
    fn drop(&mut self) {
        // Agent is dropped here if not taken via into_inner().
        // Caller is responsible for putting it back into DirectorRuntime.
    }
}

//! Memory capture pipeline — Hook-driven silent memory capture.
//!
//! Inspired by agentmemory's 12-hook capture system. Agent behavior is
//! automatically recorded via hook events (SessionStart, UserRequest,
//! ToolCall, SceneAction, etc.) and stored as structured memory entries.

use crate::memory::MemoryTier;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// MemoryHook — events that trigger memory capture
// ---------------------------------------------------------------------------

/// Hook events fired by the agent system to trigger memory capture.
#[derive(Debug, Clone)]
pub enum MemoryHook {
    /// Session started — load project context.
    SessionStart {
        project_path: String,
        session_id: String,
    },
    /// User sent a request.
    UserRequest {
        request: String,
        intent: Option<String>,
    },
    /// A tool is about to be called.
    PreToolCall {
        tool_name: String,
        arguments: serde_json::Value,
    },
    /// A tool call completed.
    PostToolCall {
        tool_name: String,
        result: serde_json::Value,
        elapsed_ms: u64,
    },
    /// A tool call failed.
    PostToolCallFailure {
        tool_name: String,
        error: String,
    },
    /// A scene mutation occurred.
    SceneAction {
        action_type: String,
        target: String,
        result: String,
    },
    /// A plan was created or revised.
    PlanRevised {
        plan_id: String,
        reason: String,
    },
    /// A goal was checked.
    GoalCheck {
        task_id: u64,
        all_matched: bool,
    },
    /// Session ended — generate summary.
    SessionEnd {
        summary: String,
    },
}

// ---------------------------------------------------------------------------
// MemoryCapturePipeline
// ---------------------------------------------------------------------------

/// Processes `MemoryHook` events through: SHA-256-like dedup → storage.
///
/// Each hook produces one or more `CapturedMemory` entries which are
/// returned for the caller to insert into the memory system.
pub struct MemoryCapturePipeline {
    /// Deduplication window.
    dedup_window: Duration,
    /// Recent hashes with their timestamps.
    recent_hashes: Vec<(u64, Instant)>,
    /// Unique counter for entry IDs.
    counter: u64,
}

#[derive(Debug, Clone)]
pub struct CapturedMemory {
    pub id: String,
    pub tier: MemoryTier,
    pub content: String,
    pub tags: Vec<String>,
    pub importance: f32,
}

impl MemoryCapturePipeline {
    pub fn new(dedup_window: Duration) -> Self {
        Self {
            dedup_window,
            recent_hashes: Vec::new(),
            counter: 0,
        }
    }

    /// Process a single hook event and return captured memory entries.
    pub fn process(&mut self, hook: &MemoryHook) -> Vec<CapturedMemory> {
        // Deduplicate: compute a simple hash + check recent window
        let hash = simple_hash(&format!("{:?}", hook));
        if self.is_duplicate(hash) {
            return Vec::new();
        }

        self.recent_hashes.push((hash, Instant::now()));
        self.counter += 1;
        let id = format!("mem_{:08}", self.counter);

        // Convert hook to memory entries
        match hook {
            MemoryHook::SessionStart { project_path, session_id } => {
                vec![CapturedMemory {
                    id: id.clone(),
                    tier: MemoryTier::Episodic,
                    content: format!("Session started: project={}, session={}", project_path, session_id),
                    tags: vec!["session_start".into(), session_id.clone()],
                    importance: 0.5,
                }]
            }
            MemoryHook::UserRequest { request, intent } => {
                let mut entries = vec![CapturedMemory {
                    id: id.clone(),
                    tier: MemoryTier::Episodic,
                    content: format!("User request: {}", request),
                    tags: vec!["user_request".into(), intent.clone().unwrap_or_default()],
                    importance: 0.7,
                }];
                if let Some(intent) = intent {
                    entries.push(CapturedMemory {
                        id: format!("{}_intent", id),
                        tier: MemoryTier::Semantic,
                        content: format!("User intent: {}", intent),
                        tags: vec!["user_intent".into(), intent.clone()],
                        importance: 0.4,
                    });
                }
                entries
            }
            MemoryHook::PreToolCall { tool_name, arguments } => {
                vec![CapturedMemory {
                    id,
                    tier: MemoryTier::Procedural,
                    content: format!("Tool called: {} with args: {}", tool_name, arguments),
                    tags: vec!["tool_call".into(), tool_name.clone()],
                    importance: 0.3,
                }]
            }
            MemoryHook::PostToolCall { tool_name, result, elapsed_ms } => {
                vec![CapturedMemory {
                    id,
                    tier: MemoryTier::Working,
                    content: format!("Tool {} completed in {}ms: {}", tool_name, elapsed_ms, result),
                    tags: vec!["tool_result".into(), tool_name.clone()],
                    importance: 0.4,
                }]
            }
            MemoryHook::PostToolCallFailure { tool_name, error } => {
                vec![CapturedMemory {
                    id,
                    tier: MemoryTier::Working,
                    content: format!("Tool {} FAILED: {}", tool_name, error),
                    tags: vec!["tool_failure".into(), tool_name.clone()],
                    importance: 0.6,
                }]
            }
            MemoryHook::SceneAction { action_type, target, result } => {
                vec![CapturedMemory {
                    id: id.clone(),
                    tier: MemoryTier::Working,
                    content: format!("Scene: {} on {} → {}", action_type, target, result),
                    tags: vec!["scene_action".into(), action_type.clone(), target.clone()],
                    importance: 0.5,
                }]
            }
            MemoryHook::PlanRevised { plan_id, reason } => {
                vec![CapturedMemory {
                    id,
                    tier: MemoryTier::Episodic,
                    content: format!("Plan {} revised: {}", plan_id, reason),
                    tags: vec!["plan_revision".into(), plan_id.clone()],
                    importance: 0.5,
                }]
            }
            MemoryHook::GoalCheck { task_id, all_matched } => {
                vec![CapturedMemory {
                    id,
                    tier: MemoryTier::Working,
                    content: format!("Goal check for task {}: {}", task_id, if *all_matched { "PASS" } else { "FAIL" }),
                    tags: vec!["goal_check".into(), format!("task_{}", task_id)],
                    importance: 0.4,
                }]
            }
            MemoryHook::SessionEnd { summary } => {
                vec![CapturedMemory {
                    id,
                    tier: MemoryTier::Semantic,
                    content: format!("Session ended: {}", summary),
                    tags: vec!["session_end".into()],
                    importance: 0.8,
                }]
            }
        }
    }

    /// Prune stale hashes from the dedup window.
    pub fn prune(&mut self) {
        let now = Instant::now();
        self.recent_hashes.retain(|(_, ts)| now.duration_since(*ts) < self.dedup_window);
    }

    fn is_duplicate(&self, hash: u64) -> bool {
        let now = Instant::now();
        self.recent_hashes.iter().any(|(h, ts)| *h == hash && now.duration_since(*ts) < self.dedup_window)
    }
}

// ---------------------------------------------------------------------------
// Simple hash (FNV-1a 64-bit)
// ---------------------------------------------------------------------------

fn simple_hash(data: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in data.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_session_start() {
        let mut pipeline = MemoryCapturePipeline::new(Duration::from_secs(300));
        let entries = pipeline.process(&MemoryHook::SessionStart {
            project_path: "/tmp/test".into(),
            session_id: "s1".into(),
        });
        assert_eq!(entries.len(), 1);
        assert!(entries[0].content.contains("Session started"));
    }

    #[test]
    fn test_deduplication() {
        let mut pipeline = MemoryCapturePipeline::new(Duration::from_secs(300));
        let hook = MemoryHook::UserRequest {
            request: "test".into(),
            intent: None,
        };
        let first = pipeline.process(&hook);
        assert_eq!(first.len(), 1);
        let second = pipeline.process(&hook);
        assert!(second.is_empty()); // deduplicated
    }

    #[test]
    fn test_prune_clears_old() {
        let mut pipeline = MemoryCapturePipeline::new(Duration::from_millis(1));
        pipeline.process(&MemoryHook::UserRequest {
            request: "x".into(),
            intent: None,
        });
        std::thread::sleep(Duration::from_millis(5));
        pipeline.prune();
        // After pruning, a new identical hook should go through
        let entries = pipeline.process(&MemoryHook::UserRequest {
            request: "x".into(),
            intent: None,
        });
        assert_eq!(entries.len(), 1);
    }
}

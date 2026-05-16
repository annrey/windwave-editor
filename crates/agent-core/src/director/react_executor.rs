//! ReAct Executor - LLM-driven think-act-observe execution engine (Framework)
//!
//! Sprint 1: This module provides the **architecture framework** for
//! ReAct ↔ DirectorRuntime integration. The actual execution logic is
//! currently in `execution.rs::execute_with_react()` and will be
//! migrated here in Sprint 1.5.
//!
//! ## Current Status
//!
//! ✅ **Completed**:
//! - ReActEvent enum (streamable events for UI)
//! - EventChannel (async communication infrastructure)
//! - ReActExecutorConfig (configuration structure)
//!
//! 🚧 **Planned (Sprint 1.5)**:
//! - Full ReActExecutor implementation (migrate from execution.rs)
//! - Complete error handling and retry logic
//! - Performance optimization

use crate::event::EventBus;
use crate::metrics::AgentMetrics;
use super::types::EditorEvent;
use tokio::sync::mpsc;

// ===========================================================================
// ReActEvent - Streamable events for UI consumption
// ===========================================================================

/// Events emitted during ReAct execution for real-time UI updates.
#[derive(Debug, Clone)]
pub enum ReActEvent {
    /// Agent is thinking about the problem.
    Thought {
        content: String,
        reasoning: String,
        step_number: usize,
    },
    /// Agent decided to call a tool.
    Action {
        tool_name: String,
        parameters: serde_json::Value,
        step_number: usize,
    },
    /// Tool execution result (observation).
    Observation {
        content: String,
        success: bool,
        step_number: usize,
    },
    /// Agent reached a final answer.
    FinalAnswer {
        content: String,
        total_steps: usize,
    },
    /// Execution error (LLM timeout, tool failure, etc.).
    Error {
        message: String,
        step_number: usize,
        is_recoverable: bool,
    },
    /// Execution completed (success or failure).
    Completed {
        success: bool,
        total_steps: usize,
        duration_ms: u64,
        final_response: String,
    },
}

impl ReActEvent {
    /// Convert to EditorEvent for backward compatibility with existing UI.
    pub fn to_editor_event(&self, request: &str) -> EditorEvent {
        match self {
            ReActEvent::Thought { step_number, .. } => {
                EditorEvent::DirectExecutionStarted {
                    request: request.to_string(),
                    mode: format!("ReAct-Think#{}", step_number),
                    complexity_score: 5,
                }
            }
            ReActEvent::Action { tool_name, step_number, .. } => {
                EditorEvent::StepStarted {
                    plan_id: "react_loop".to_string(),
                    step_id: format!("step_{}", step_number),
                    title: format!("Call {}", tool_name),
                }
            }
            ReActEvent::Observation { success, step_number, .. } => {
                if *success {
                    EditorEvent::StepCompleted {
                        plan_id: "react_loop".to_string(),
                        step_id: format!("step_{}", step_number),
                        title: format!("Observation#{}", step_number),
                        result: "Tool executed successfully".to_string(),
                    }
                } else {
                    EditorEvent::StepFailed {
                        plan_id: "react_loop".to_string(),
                        step_id: format!("step_{}", step_number),
                        title: format!("Observation#{}", step_number),
                        error: "Tool execution failed".to_string(),
                    }
                }
            }
            ReActEvent::FinalAnswer { .. } => {
                EditorEvent::DirectExecutionCompleted {
                    request: request.to_string(),
                    success: true,
                }
            }
            ReActEvent::Error { message, .. } => {
                EditorEvent::Error {
                    message: message.clone(),
                }
            }
            ReActEvent::Completed { success, .. } => {
                EditorEvent::DirectExecutionCompleted {
                    request: request.to_string(),
                    success: *success,
                }
            }
        }
    }
}

// ===========================================================================
// EventChannel - Async channel for streaming events
// ===========================================================================

/// Bidirectional event channel for ReAct ↔ DirectorRuntime communication.
pub struct EventChannel {
    sender: mpsc::UnboundedSender<ReActEvent>,
}

impl EventChannel {
    /// Create a new event channel pair.
    pub fn new() -> (Self, mpsc::UnboundedReceiver<ReActEvent>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (Self { sender }, receiver)
    }

    /// Push an event to the channel (non-blocking).
    pub fn push(&self, event: ReActEvent) -> Result<(), String> {
        self.sender.send(event)
            .map_err(|e| format!("EventChannel closed: {}", e))
    }

    /// Push a Thought event.
    pub fn push_thought(&self, content: &str, reasoning: &str, step: usize) -> Result<(), String> {
        self.push(ReActEvent::Thought {
            content: content.to_string(),
            reasoning: reasoning.to_string(),
            step_number: step,
        })
    }

    /// Push an Action event.
    pub fn push_action(&self, tool_name: &str, params: &serde_json::Value, step: usize) -> Result<(), String> {
        self.push(ReActEvent::Action {
            tool_name: tool_name.to_string(),
            parameters: params.clone(),
            step_number: step,
        })
    }

    /// Push an Observation event.
    pub fn push_observation(&self, content: &str, success: bool, step: usize) -> Result<(), String> {
        self.push(ReActEvent::Observation {
            content: content.to_string(),
            success,
            step_number: step,
        })
    }

    /// Push an Error event.
    pub fn push_error(&self, message: &str, step: usize, recoverable: bool) -> Result<(), String> {
        self.push(ReActEvent::Error {
            message: message.to_string(),
            step_number: step,
            is_recoverable: recoverable,
        })
    }

    /// Push completion event.
    pub fn push_completed(&self, success: bool, steps: usize, duration_ms: u64, response: &str) -> Result<(), String> {
        self.push(ReActEvent::Completed {
            success,
            total_steps: steps,
            duration_ms,
            final_response: response.to_string(),
        })
    }
}

// ===========================================================================
// ReActExecutor Configuration
// ===========================================================================

/// Configuration for the ReAct execution engine.
#[derive(Debug, Clone)]
pub struct ReActExecutorConfig {
    /// Maximum number of ReAct steps before forced completion.
    pub max_steps: usize,
    /// Timeout per LLM call (in seconds).
    pub llm_timeout_secs: u64,
    /// Whether to automatically retry failed tool calls.
    pub auto_retry_failed_tools: bool,
    /// Maximum retries per tool call.
    pub max_tool_retries: usize,
    /// Whether to stream events in real-time (vs batch at end).
    pub stream_events: bool,
}

impl Default for ReActExecutorConfig {
    fn default() -> Self {
        Self {
            max_steps: 20,
            llm_timeout_secs: 30,
            auto_retry_failed_tools: true,
            max_tool_retries: 2,
            stream_events: true,
        }
    }
}

// ===========================================================================
// Framework Placeholder - Full implementation in Sprint 1.5
// ===========================================================================

/// ReAct Executor - **Framework placeholder** for future migration.
///
/// The complete implementation currently lives in:
/// - `execution.rs::execute_with_react()` - Main ReAct loop
/// - `execution.rs::execute_react_tool()` - Tool execution
/// - `execution.rs::observe_and_continue()` - Observation feedback
///
/// This struct will be fully implemented in Sprint 1.5 to provide:
/// - Cleaner separation of concerns
/// - Better testability
/// - Independent lifecycle management
pub struct ReActExecutor<'a> {
    /// Placeholder for future use
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> ReActExecutor<'a> {
    /// Create a new ReAct executor placeholder.
    ///
    /// **Note**: This is a framework stub. Use `DirectorRuntime::execute_with_react()`
    /// for actual ReAct execution in Sprint 1.
    pub fn new(
        _react_agent: &mut crate::strategy::ReActAgent,
        _scene_bridge: Option<&'a mut dyn crate::scene_bridge::SceneBridge>,
        _config: ReActExecutorConfig,
        _event_channel: EventChannel,
        _event_bus: EventBus,
        _metrics: AgentMetrics,
        _request_text: String,
    ) -> Self {
        eprintln!("[ReActExecutor] Framework placeholder created - full implementation in Sprint 1.5");
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

// Tests disabled - see execution.rs::acceptance_tests for working tests

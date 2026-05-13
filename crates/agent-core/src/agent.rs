//! BaseAgent - Core Agent framework with state machine
//!
//! Following OpenManus design: BaseAgent provides the foundation with
//! state management, step execution, and error handling.

use crate::types::*;
use crate::memory_legacy::{ConversationMemory, SessionMemory};
use std::time::Duration;
use chrono::{DateTime, Utc};

/// Unique identifier for Agent instances
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AgentInstanceId(pub u64);

/// Agent state machine - tracks the current phase of operation
#[derive(Debug, Clone)]
pub enum AgentState {
    /// Idle - waiting for input
    Idle {
        last_activity: DateTime<Utc>,
    },
    
    /// Analyzing the user request
    AnalyzingRequest {
        request: UserRequest,
        start_time: DateTime<Utc>,
    },
    
    /// Planning phase (for complex tasks)
    Planning {
        plan_id: String,
        current_step: usize,
        total_steps: usize,
    },
    
    /// Thinking about next action
    Thinking {
        context_hash: String,
        iteration: usize,
    },
    
    /// Selecting appropriate tools
    SelectingTools {
        available: Vec<String>,
        selected: Vec<String>,
    },
    
    /// Executing tools/actions
    ExecutingTools {
        completed: usize,
        in_progress: usize,
        total: usize,
    },
    
    /// Waiting for user confirmation
    WaitingForConfirmation {
        pending_action: String,
        timeout: DateTime<Utc>,
    },
    
    /// Observing results
    Observing {
        results_summary: String,
    },
    
    /// Task completed successfully
    Finished {
        result: AgentResult,
        final_message: String,
    },
    
    /// Error state
    Error {
        error: String,
        recoverable: bool,
    },
    
    /// Stuck - detected loop or no progress
    Stuck {
        reason: StuckReason,
        last_progress: DateTime<Utc>,
    },
}

/// Reasons why Agent might be stuck
#[derive(Debug, Clone)]
pub enum StuckReason {
    LoopingThoughts,      // Repeating the same thought pattern
    RepeatedToolCalls,    // Calling same tools with same params
    NoProgress,           // No meaningful progress for too long
    ConflictingDecisions, // Contradictory decisions
}

/// Configuration for BaseAgent
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Maximum number of steps before forced stop
    pub max_steps: usize,
    /// Timeout for single step
    pub step_timeout: Duration,
    /// Timeout for entire execution
    pub execution_timeout: Duration,
    /// Number of recent steps to check for loops
    pub cycle_detection_window: usize,
    /// Whether to require user confirmation for actions
    pub require_confirmation: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_steps: 50,
            step_timeout: Duration::from_secs(30),
            execution_timeout: Duration::from_secs(300),
            cycle_detection_window: 3,
            require_confirmation: true,
        }
    }
}

/// Result of a single step
#[derive(Debug, Clone)]
pub struct StepResult {
    pub thought: String,
    pub action: String,
    pub observation: String,
    pub completed: bool,
}

/// Final result from Agent execution
#[derive(Debug, Clone)]
pub struct AgentResult {
    pub success: bool,
    pub message: String,
    pub steps_executed: usize,
    pub actions_performed: Vec<AgentAction>,
}

/// BaseAgent - Foundation of the Agent system
/// 
/// Implements the core request-response cycle with state management,
/// error handling, and stuck detection.
pub struct BaseAgent {
    /// Identity
    pub id: AgentInstanceId,
    pub name: String,
    pub description: String,
    
    /// Configuration
    pub config: AgentConfig,
    
    /// Current state
    pub state: AgentState,
    
    /// Execution tracking
    pub current_step: usize,
    pub step_history: Vec<StepResult>,
    
    /// Memory systems
    pub conversation_memory: ConversationMemory,
    pub session_memory: SessionMemory,
    
    /// Callback hooks
    pub on_step_start: Option<Box<dyn Fn(&AgentState) + Send + Sync>>,
    pub on_step_end: Option<Box<dyn Fn(&StepResult) + Send + Sync>>,
    pub on_state_change: Option<Box<dyn Fn(&AgentState, &AgentState) + Send + Sync>>,
}

impl BaseAgent {
    /// Create a new BaseAgent
    pub fn new(id: AgentInstanceId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            description: String::new(),
            config: AgentConfig::default(),
            state: AgentState::Idle { 
                last_activity: Utc::now() 
            },
            current_step: 0,
            step_history: Vec::new(),
            conversation_memory: ConversationMemory::new(10),
            session_memory: SessionMemory::new(),
            on_step_start: None,
            on_step_end: None,
            on_state_change: None,
        }
    }
    
    /// Configure the agent
    pub fn with_config(mut self, config: AgentConfig) -> Self {
        self.config = config;
        self
    }
    
    /// Set description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }
    
    /// Main execution entry point
    /// 
    /// Runs the Agent loop: think -> act -> observe -> repeat
    /// until completion or error
    pub async fn run(&mut self, request: UserRequest) -> Result<AgentResult, AgentError> {
        // Initialize execution
        self.current_step = 0;
        self.step_history.clear();
        
        // Store request in conversation memory
        self.conversation_memory.add_message(Message::new_user(&request.content));
        
        // Transition to analyzing state
        self.transition_to(AgentState::AnalyzingRequest {
            request: request.clone(),
            start_time: Utc::now(),
        });
        
        // Main execution loop
        loop {
            // Check max steps
            if self.current_step >= self.config.max_steps {
                return Err(AgentError::MaxStepsReached);
            }
            
            // Check for stuck state
            if self.is_stuck() {
                self.handle_stuck_state().await?;
            }
            
            // Execute single step
            let step_result = self.step().await?;
            self.current_step += 1;
            self.step_history.push(step_result.clone());
            
            // Notify step end callback
            if let Some(ref callback) = self.on_step_end {
                callback(&step_result);
            }
            
            // Check if finished
            if step_result.completed {
                return Ok(AgentResult {
                    success: true,
                    message: step_result.observation,
                    steps_executed: self.current_step,
                    actions_performed: Vec::new(), // Populated by subclasses
                });
            }
        }
    }
    
    /// Single step execution - to be implemented by subclasses
    /// 
    /// Default implementation provides a basic think-act-observe cycle
    async fn step(&mut self) -> Result<StepResult, AgentError> {
        // Notify step start
        if let Some(ref callback) = self.on_step_start {
            callback(&self.state);
        }
        
        // 1. Think - analyze and decide
        self.transition_to(AgentState::Thinking {
            context_hash: self.compute_context_hash(),
            iteration: self.current_step,
        });
        let thought = self.think().await?;
        
        // 2. Act - perform action (subclass implements)
        let action = self.act(&thought).await?;
        
        // 3. Observe - process results
        self.transition_to(AgentState::Observing {
            results_summary: action.clone(),
        });
        let observation = self.observe(&action).await?;
        
        // Store in memory
        self.conversation_memory.add_message(Message::thought(&thought));
        self.conversation_memory.add_message(Message::action(&action));
        self.conversation_memory.add_message(Message::observation(&observation));
        
        Ok(StepResult {
            thought,
            action,
            observation: observation.clone(),
            completed: self.detect_completion(&observation),
        })
    }
    
    /// Think phase - analyze current state and decide next action
    /// 
    /// Subclasses should override this to integrate with LLM
    async fn think(&self) -> Result<String, AgentError> {
        // Default: simple echo of request
        let recent = self.conversation_memory.recent_messages(3);
        let context = recent.iter()
            .map(|m| format!("{:?}: {}", m.message_type, m.content))
            .collect::<Vec<_>>()
            .join("\n");
        
        Ok(format!("Analyzing: {}", context))
    }
    
    /// Act phase - execute the decided action
    /// 
    /// Must be implemented by subclasses
    async fn act(&self, thought: &str) -> Result<String, AgentError> {
        // Default: just echo the thought
        Ok(format!("Action based on: {}", thought))
    }
    
    /// Observe phase - process action results
    async fn observe(&self, action_result: &str) -> Result<String, AgentError> {
        // Default: simple observation
        Ok(format!("Observed result: {}", action_result))
    }
    
    /// Detect if task is complete based on observation
    fn detect_completion(&self, observation: &str) -> bool {
        // Default: check for completion keywords
        observation.to_lowercase().contains("completed") 
            || observation.to_lowercase().contains("done")
            || self.current_step >= 5 // Safety limit for base implementation
    }
    
    /// Check if Agent is stuck in a loop
    fn is_stuck(&self) -> bool {
        if self.step_history.len() < self.config.cycle_detection_window {
            return false;
        }
        
        let recent = &self.step_history[
            self.step_history.len() - self.config.cycle_detection_window..
        ];
        
        // Check for repeating thoughts (cycle detection)
        let thoughts: Vec<_> = recent.iter().map(|s| &s.thought).collect();
        if thoughts.windows(2).any(|w| w[0] == w[1]) {
            return true;
        }
        
        // Check for repeating actions
        let actions: Vec<_> = recent.iter().map(|s| &s.action).collect();
        if actions.windows(2).any(|w| w[0] == w[1]) {
            return true;
        }
        
        false
    }
    
    /// Handle stuck state - attempt recovery
    async fn handle_stuck_state(&mut self) -> Result<(), AgentError> {
        self.transition_to(AgentState::Stuck {
            reason: StuckReason::LoopingThoughts,
            last_progress: Utc::now(),
        });
        
        // Strategy: clear recent memory and try a different approach
        self.conversation_memory.clear_recent(3);
        
        // Add escape message
        self.conversation_memory.add_message(
            Message::new_agent("Detected loop. Trying alternative approach.")
        );
        
        Ok(())
    }
    
    /// Transition to a new state
    pub fn transition_to(&mut self, new_state: AgentState) {
        let old_state = std::mem::replace(&mut self.state, new_state);
        
        // Notify state change callback
        if let Some(ref callback) = self.on_state_change {
            callback(&old_state, &self.state);
        }
    }
    
    /// Compute a hash of current context for cycle detection
    fn compute_context_hash(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        self.current_step.hash(&mut hasher);
        self.conversation_memory.message_count().hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
    
    /// Get current state display name
    pub fn state_name(&self) -> &'static str {
        match &self.state {
            AgentState::Idle { .. } => "Idle",
            AgentState::AnalyzingRequest { .. } => "Analyzing",
            AgentState::Planning { .. } => "Planning",
            AgentState::Thinking { .. } => "Thinking",
            AgentState::SelectingTools { .. } => "Selecting Tools",
            AgentState::ExecutingTools { .. } => "Executing",
            AgentState::WaitingForConfirmation { .. } => "Waiting",
            AgentState::Observing { .. } => "Observing",
            AgentState::Finished { .. } => "Finished",
            AgentState::Error { .. } => "Error",
            AgentState::Stuck { .. } => "Stuck",
        }
    }
    
    /// Get progress percentage (if available)
    pub fn progress(&self) -> Option<f32> {
        match &self.state {
            AgentState::Planning { current_step, total_steps, .. } => {
                Some(*current_step as f32 / *total_steps as f32)
            }
            AgentState::ExecutingTools { completed, total, .. } => {
                Some(*completed as f32 / *total as f32)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_agent_creation() {
        let agent = BaseAgent::new(AgentInstanceId(1), "TestAgent");
        assert_eq!(agent.name, "TestAgent");
        assert_eq!(agent.current_step, 0);
        assert!(matches!(agent.state, AgentState::Idle { .. }));
    }
    
    #[test]
    fn test_stuck_detection() {
        let mut agent = BaseAgent::new(AgentInstanceId(1), "TestAgent");
        agent.config.cycle_detection_window = 3;
        
        // Add repeating steps
        agent.step_history.push(StepResult {
            thought: "same thought".to_string(),
            action: "action 1".to_string(),
            observation: "obs 1".to_string(),
            completed: false,
        });
        agent.step_history.push(StepResult {
            thought: "same thought".to_string(), // repeated
            action: "action 2".to_string(),
            observation: "obs 2".to_string(),
            completed: false,
        });
        agent.step_history.push(StepResult {
            thought: "different".to_string(),
            action: "action 3".to_string(),
            observation: "obs 3".to_string(),
            completed: false,
        });
        
        assert!(agent.is_stuck());
    }
}

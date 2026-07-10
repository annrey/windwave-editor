//! Workflow Module
//!
//! Provides multi-agent collaboration workflow capabilities.

use crate::error::{Error, Result};
use crate::types::{AgentConfig, WorkflowResult, WorkflowStep};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

/// Status of a workflow execution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowStatus {
    /// Workflow is idle
    Idle,
    /// Workflow is running
    Running,
    /// Workflow has been paused
    Paused,
    /// Workflow completed successfully
    Completed,
    /// Workflow failed
    Failed { error: String },
}

/// Execution state for a single step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepExecutionState {
    pub step: WorkflowStep,
    pub status: StepStatus,
    pub result: Option<String>,
    pub error: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

/// Status of an individual workflow step
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    /// Step waiting to be executed
    Pending,
    /// Step is currently executing
    Running,
    /// Step completed successfully
    Completed,
    /// Step failed
    Failed,
    /// Step was skipped
    Skipped,
}

/// Workflow Manager for orchestrating multi-step, multi-agent workflows
pub struct WorkflowManager {
    /// Registered agents that can be used in workflows
    agents: HashMap<String, AgentConfig>,
    /// Active workflow executions
    executions: HashMap<String, WorkflowExecution>,
    /// Execution history
    history: VecDeque<WorkflowExecutionRecord>,
    /// Maximum history size
    max_history: usize,
}

/// Record of a workflow execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecutionRecord {
    pub execution_id: String,
    pub workflow_name: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub status: WorkflowStatus,
    pub step_count: usize,
}

/// State of an active workflow execution
#[derive(Debug, Clone)]
pub struct WorkflowExecution {
    pub execution_id: String,
    pub workflow_name: String,
    pub steps: Vec<StepExecutionState>,
    pub status: WorkflowStatus,
    pub start_time: String,
    pub current_step_index: usize,
}

impl WorkflowManager {
    /// Create a new workflow manager
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            executions: HashMap::new(),
            history: VecDeque::new(),
            max_history: 100,
        }
    }

    /// Register an agent with the workflow manager
    pub fn register_agent(&mut self, agent: AgentConfig) {
        self.agents.insert(agent.name.clone(), agent);
    }

    /// List all registered agents
    pub fn list_agents(&self) -> Vec<&AgentConfig> {
        self.agents.values().collect()
    }

    /// Start a new workflow execution
    pub fn start_workflow(
        &mut self,
        workflow_name: String,
        steps: Vec<WorkflowStep>,
    ) -> Result<String> {
        let execution_id = format!("workflow_{}", uuid::Uuid::new_v4());

        let step_states = steps
            .into_iter()
            .map(|step| StepExecutionState {
                step,
                status: StepStatus::Pending,
                result: None,
                error: None,
                started_at: None,
                completed_at: None,
            })
            .collect();

        let execution = WorkflowExecution {
            execution_id: execution_id.clone(),
            workflow_name,
            steps: step_states,
            status: WorkflowStatus::Idle,
            start_time: chrono::Utc::now().to_rfc3339(),
            current_step_index: 0,
        };

        self.executions.insert(execution_id.clone(), execution);
        Ok(execution_id)
    }

    /// Check if all dependencies for a step are met
    fn check_dependencies(steps: &[StepExecutionState], step_index: usize) -> bool {
        let step = &steps[step_index];

        for dep in &step.step.dependencies {
            let dep_index = steps.iter().position(|s| s.step.name == *dep);

            match dep_index {
                Some(i) => {
                    let dep_step = &steps[i];
                    if dep_step.status != StepStatus::Completed {
                        return false;
                    }
                }
                None => {
                    return false; // Dependency not found
                }
            }
        }

        true
    }

    /// Execute an individual workflow step
    async fn execute_step(&self, step: &WorkflowStep) -> Result<String> {
        let agent_name = &step.agent;

        if !self.agents.contains_key(agent_name) {
            return Err(Error::ConfigError(format!(
                "Agent not found: {}",
                agent_name
            )));
        }

        let output = format!(
            "Agent {} executed step: {} with input: {}",
            agent_name, step.name, step.input
        );

        Ok(output)
    }

    /// Execute a workflow step-by-step
    pub async fn execute_workflow(&mut self, execution_id: &str) -> Result<WorkflowResult> {
        // Get and remove execution from map to avoid borrow conflicts
        let mut execution = self
            .executions
            .remove(execution_id)
            .ok_or_else(|| Error::ConfigError(format!("Execution not found: {}", execution_id)))?;

        execution.status = WorkflowStatus::Running;

        let mut final_output = String::new();
        let mut failed = false;
        let mut error_msg = String::new();

        for i in execution.current_step_index..execution.steps.len() {
            // First, check dependencies without mutable borrow
            let should_skip = {
                let step = &execution.steps[i];
                step.status != StepStatus::Completed
                    && !Self::check_dependencies(&execution.steps, i)
            };

            let step = &mut execution.steps[i];

            if should_skip {
                step.status = StepStatus::Skipped;
                step.completed_at = Some(chrono::Utc::now().to_rfc3339());
                continue;
            }

            step.status = StepStatus::Running;
            step.started_at = Some(chrono::Utc::now().to_rfc3339());

            // Execute the step
            match self.execute_step(&step.step).await {
                Ok(output) => {
                    step.status = StepStatus::Completed;
                    step.result = Some(output.clone());
                    final_output = output;
                }
                Err(e) => {
                    step.status = StepStatus::Failed;
                    step.error = Some(e.to_string());
                    execution.status = WorkflowStatus::Failed {
                        error: e.to_string(),
                    };
                    failed = true;
                    error_msg = e.to_string();
                }
            }

            step.completed_at = Some(chrono::Utc::now().to_rfc3339());
            execution.current_step_index = i + 1;

            if failed {
                break;
            }
        }

        if !failed {
            execution.status = WorkflowStatus::Completed;
        }

        // Record in history
        self.record_execution(&execution);

        if failed {
            return Err(Error::ConfigError(error_msg));
        }

        Ok(WorkflowResult {
            steps: execution
                .steps
                .iter()
                .map(|s| crate::types::StepResult {
                    step_name: s.step.name.clone(),
                    output: s.result.clone().unwrap_or_default(),
                    success: s.status == StepStatus::Completed,
                    error: s.error.clone(),
                })
                .collect(),
            final_output,
        })
    }

    /// Record an execution in history
    fn record_execution(&mut self, execution: &WorkflowExecution) {
        let record = WorkflowExecutionRecord {
            execution_id: execution.execution_id.clone(),
            workflow_name: execution.workflow_name.clone(),
            start_time: execution.start_time.clone(),
            end_time: Some(chrono::Utc::now().to_rfc3339()),
            status: execution.status.clone(),
            step_count: execution.steps.len(),
        };

        self.history.push_front(record);

        if self.history.len() > self.max_history {
            self.history.pop_back();
        }

        // Only remove from active executions if completed/failed
        if matches!(
            execution.status,
            WorkflowStatus::Completed | WorkflowStatus::Failed { .. }
        ) {
            // Already removed during execute_workflow
        } else {
            // Put back if not completed
            self.executions
                .insert(execution.execution_id.clone(), execution.clone());
        }
    }

    /// Get the status of a workflow execution
    pub fn get_execution_status(&self, execution_id: &str) -> Option<&WorkflowStatus> {
        self.executions
            .get(execution_id)
            .map(|e| &e.status)
            .or_else(|| {
                self.history
                    .iter()
                    .find(|r| r.execution_id == execution_id)
                    .map(|r| &r.status)
            })
    }

    /// Get execution history
    pub fn get_history(&self) -> Vec<&WorkflowExecutionRecord> {
        self.history.iter().collect()
    }

    /// Pause a running workflow
    pub fn pause_workflow(&mut self, execution_id: &str) -> Result<()> {
        let execution = self
            .executions
            .get_mut(execution_id)
            .ok_or_else(|| Error::ConfigError(format!("Execution not found: {}", execution_id)))?;

        if execution.status == WorkflowStatus::Running {
            execution.status = WorkflowStatus::Paused;
            Ok(())
        } else {
            Err(Error::ConfigError(
                "Workflow is not in a running state".to_string(),
            ))
        }
    }

    /// Resume a paused workflow
    pub async fn resume_workflow(&mut self, execution_id: &str) -> Result<WorkflowResult> {
        // First update status to running
        {
            let execution = self.executions.get_mut(execution_id).ok_or_else(|| {
                Error::ConfigError(format!("Execution not found: {}", execution_id))
            })?;

            if execution.status != WorkflowStatus::Paused {
                return Err(Error::ConfigError(
                    "Workflow is not in a paused state".to_string(),
                ));
            }

            execution.status = WorkflowStatus::Running;
        }

        self.execute_workflow(execution_id).await
    }

    /// Cancel a workflow
    pub fn cancel_workflow(&mut self, execution_id: &str) -> Result<()> {
        if let Some(mut execution) = self.executions.remove(execution_id) {
            execution.status = WorkflowStatus::Failed {
                error: "Cancelled by user".to_string(),
            };
            self.record_execution(&execution);
            Ok(())
        } else {
            Err(Error::ConfigError(format!(
                "Execution not found: {}",
                execution_id
            )))
        }
    }

    /// Get active executions
    pub fn get_active_executions(&self) -> Vec<&WorkflowExecution> {
        self.executions.values().collect()
    }
}

impl Default for WorkflowManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_manager_creation() {
        let manager = WorkflowManager::new();
        assert!(manager.get_active_executions().is_empty());
    }

    #[test]
    fn test_register_agent() {
        let mut manager = WorkflowManager::new();

        let agent = AgentConfig {
            name: "test-agent".to_string(),
            system_prompt: "You are a test agent".to_string(),
            tools: vec![],
            framework: crate::types::FrameworkType::LangChain,
        };

        manager.register_agent(agent);
        assert_eq!(manager.list_agents().len(), 1);
    }

    #[test]
    fn test_start_workflow() {
        let mut manager = WorkflowManager::new();

        // Register agent first
        let agent = AgentConfig {
            name: "test-agent".to_string(),
            system_prompt: "You are a test agent".to_string(),
            tools: vec![],
            framework: crate::types::FrameworkType::LangChain,
        };
        manager.register_agent(agent);

        let steps = vec![crate::types::WorkflowStep {
            name: "step1".to_string(),
            agent: "test-agent".to_string(),
            input: "Hello".to_string(),
            dependencies: vec![],
        }];

        let result = manager.start_workflow("test-workflow".to_string(), steps);
        assert!(result.is_ok());
        assert_eq!(manager.get_active_executions().len(), 1);
    }

    #[test]
    fn test_cancel_workflow() {
        let mut manager = WorkflowManager::new();

        let agent = AgentConfig {
            name: "test-agent".to_string(),
            system_prompt: "You are a test agent".to_string(),
            tools: vec![],
            framework: crate::types::FrameworkType::LangChain,
        };
        manager.register_agent(agent);

        let steps = vec![crate::types::WorkflowStep {
            name: "step1".to_string(),
            agent: "test-agent".to_string(),
            input: "Hello".to_string(),
            dependencies: vec![],
        }];

        let execution_id = manager
            .start_workflow("test-workflow".to_string(), steps)
            .unwrap();
        assert!(manager.cancel_workflow(&execution_id).is_ok());
        assert_eq!(manager.get_active_executions().len(), 0);
    }

    #[test]
    fn test_workflow_history() {
        let mut manager = WorkflowManager::new();

        let agent = AgentConfig {
            name: "test-agent".to_string(),
            system_prompt: "You are a test agent".to_string(),
            tools: vec![],
            framework: crate::types::FrameworkType::LangChain,
        };
        manager.register_agent(agent);

        let steps = vec![crate::types::WorkflowStep {
            name: "step1".to_string(),
            agent: "test-agent".to_string(),
            input: "Hello".to_string(),
            dependencies: vec![],
        }];

        let execution_id = manager
            .start_workflow("test-workflow".to_string(), steps)
            .unwrap();
        let _ = manager.cancel_workflow(&execution_id);

        let history = manager.get_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].workflow_name, "test-workflow");
    }
}

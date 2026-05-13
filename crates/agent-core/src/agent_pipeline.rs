//! Agent Pipeline — unified pre/post hooks + retry + error recovery

use std::time::{Duration, Instant};
use crate::permission::OperationRisk;
use crate::tool::{ToolCall, ToolResult, ToolRegistry};

#[derive(Debug, Clone)]
pub struct PipelineConfig { pub max_retries: u32, pub retry_base_ms: u64, pub timeout_ms: u64, pub risk_threshold: OperationRisk }

impl Default for PipelineConfig {
    fn default() -> Self { Self { max_retries: 3, retry_base_ms: 200, timeout_ms: 30000, risk_threshold: OperationRisk::HighRisk } }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineError { ValidationFailed(String), PermissionDenied(String), ExecutionFailed(String), MaxRetriesExceeded(u32) }

#[derive(Debug, Clone)]
pub enum ErrorDecision { RetryAfter(Duration), Escalate(PipelineError), Ignore }

pub type PreHook = Box<dyn Fn(&mut PipelineContext) -> Result<(), PipelineError> + Send + Sync>;
pub type PostHook = Box<dyn Fn(&PipelineContext, &ToolResult) -> Result<(), PipelineError> + Send + Sync>;
pub type ErrorHandler = Box<dyn Fn(&PipelineContext, &PipelineError) -> ErrorDecision + Send + Sync>;

#[derive(Debug, Clone)]
pub struct PipelineContext { pub step: String, pub tool_call: ToolCall, pub risk: OperationRisk, pub attempt: u32 }

#[derive(Debug, Clone)]
pub struct PipelineResult { pub success: bool, pub attempts: u32, pub elapsed: Duration, pub result: Option<ToolResult>, pub error: Option<PipelineError> }

pub struct AgentPipeline { config: PipelineConfig, pre: Vec<PreHook>, post: Vec<PostHook>, err_handler: Option<ErrorHandler> }

impl AgentPipeline {
    pub fn new(config: PipelineConfig) -> Self { Self { config, pre: vec![], post: vec![], err_handler: None } }
    pub fn add_pre(&mut self, h: PreHook) { self.pre.push(h); }
    pub fn add_post(&mut self, h: PostHook) { self.post.push(h); }
    pub fn set_error_handler(&mut self, h: ErrorHandler) { self.err_handler = Some(h); }

    fn risk_ok(&self, risk: OperationRisk) -> bool { (risk as u8) <= (self.config.risk_threshold as u8) }

    pub fn execute(&self, tools: &ToolRegistry, call: ToolCall, risk: OperationRisk) -> PipelineResult {
        let start = Instant::now();
        if !self.risk_ok(risk) {
            return PipelineResult { success: false, attempts: 0, elapsed: start.elapsed(), result: None, error: Some(PipelineError::PermissionDenied("risk threshold".into())) };
        }
        let mut ctx = PipelineContext { step: "pre".into(), tool_call: call, risk, attempt: 0 };

        for attempt in 1..=self.config.max_retries {
            ctx.attempt = attempt;
            ctx.step = "pre".into();
            for hook in &self.pre {
                if let Err(e) = hook(&mut ctx) {
                    match self.handle_error(&ctx, &e) {
                        ErrorDecision::RetryAfter(d) => { std::thread::sleep(d); continue; }
                        ErrorDecision::Ignore => {},
                        ErrorDecision::Escalate(e) => return PipelineResult { success: false, attempts: attempt, elapsed: start.elapsed(), result: None, error: Some(e) },
                    }
                }
            }

            ctx.step = "exec".into();
            match tools.execute(&ctx.tool_call) {
                Ok(result) => {
                    ctx.step = "post".into();
                    for hook in &self.post {
                        if let Err(e) = hook(&ctx, &result) {
                            return PipelineResult { success: false, attempts: attempt, elapsed: start.elapsed(), result: None, error: Some(e) };
                        }
                    }
                    return PipelineResult { success: true, attempts: attempt, elapsed: start.elapsed(), result: Some(result), error: None };
                }
                Err(_) => {
                    if attempt < self.config.max_retries {
                        std::thread::sleep(Duration::from_millis(self.config.retry_base_ms * 2u64.pow(attempt - 1)));
                        continue;
                    }
                    return PipelineResult { success: false, attempts: attempt, elapsed: start.elapsed(), result: None, error: Some(PipelineError::MaxRetriesExceeded(attempt)) };
                }
            }
        }
        PipelineResult { success: false, attempts: self.config.max_retries, elapsed: start.elapsed(), result: None, error: Some(PipelineError::MaxRetriesExceeded(self.config.max_retries)) }
    }

    fn handle_error(&self, ctx: &PipelineContext, err: &PipelineError) -> ErrorDecision {
        self.err_handler.as_ref().map(|h| h(ctx, err)).unwrap_or(ErrorDecision::Escalate(err.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*; use crate::tool::ToolRegistry; use std::collections::HashMap;

    #[test]
    fn test_pipeline_risk_blocked() {
        let tools = ToolRegistry::new();
        let pipeline = AgentPipeline::new(PipelineConfig { risk_threshold: OperationRisk::Safe, ..Default::default() });
        let call = ToolCall { tool_name: "e".into(), call_id: "1".into(), parameters: HashMap::new() };
        assert!(!pipeline.execute(&tools, call, OperationRisk::LowRisk).success);
    }

    #[test]
    fn test_pipeline_retry() {
        let tools = ToolRegistry::new();
        let pipeline = AgentPipeline::new(PipelineConfig { max_retries: 2, ..Default::default() });
        let call = ToolCall { tool_name: "nonexistent".into(), call_id: "1".into(), parameters: HashMap::new() };
        let r = pipeline.execute(&tools, call, OperationRisk::Safe);
        assert!(!r.success);
        assert_eq!(r.attempts, 2);
    }
}

#![allow(clippy::absurd_extreme_comparisons)]
#![allow(clippy::assertions_on_constants)]

pub mod dspy_integration;
pub mod error;
pub mod knowledge_base;
pub mod langchain_integration;
pub mod llamaindex_integration;
pub mod types;
pub mod unified_interface;
pub mod workflow;

pub use error::{Error, Result};
pub use knowledge_base::{IndexConfig, KnowledgeBaseManager};
pub use types::*;
pub use unified_interface::AIOrchestrator;
pub use workflow::{WorkflowExecutionRecord, WorkflowManager, WorkflowStatus};

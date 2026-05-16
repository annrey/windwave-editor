//! Agent Core - Phase 1: BaseAgent Framework + State Machine + Memory System
//!
//! This crate defines the foundational Agent architecture for AgentEdit,
//! following the OpenClaw + OpenManus inspired design.

pub mod search_engine;
pub mod lifecycle;
pub mod capture_pipeline;
pub mod agent;
pub mod agent_platform;
pub mod agent_pipeline;
pub mod agent_context;
pub mod audit;
pub mod bevy_editor_model;
pub mod builtin_skills;
pub mod cli_agent;
pub mod code_graph;
pub mod code_tools;
pub mod command;
pub mod context;
pub mod context_collector;
pub mod director;
pub mod dynamic_planner;
pub mod event;
pub mod event_stream;
pub mod edit_ops;
pub mod edit_history;
pub mod engine_tools;
pub mod fallback;
pub mod file_tools;
pub mod game_mode;
pub mod goal;
pub mod goal_checker;
pub mod hr_agent;
pub mod index;
pub mod llm;
pub mod mcp_registry;
pub mod memory;
pub mod message_buffer;
pub mod persistent_memory;
pub mod memory_injector;
pub mod metrics;
pub mod narrative;
pub mod router;
pub mod runtime_agent;
pub mod runtime_agent_tools;
pub mod module;
pub mod permission;
pub mod plan;
pub mod planner;
pub mod hybrid_controller;
pub mod project;
pub mod project_system;
pub mod prompt;
pub mod reflection_engine;
pub mod registry;
pub mod review;
pub mod rollback;
pub mod rule_engine;
pub mod scene_agent;
pub mod scene_bridge;
pub mod scene_serializer;
pub mod scene_tools;
pub mod selection_tools;
pub mod self_modifying_agent;
pub mod skill;
pub mod specialized_agents;
pub mod strategy;
pub mod task;
pub mod team_context;
pub mod team_structure;
pub mod transaction;
pub mod tool;
pub mod types;
pub mod vision;
pub mod visual_script;
pub mod visual_system;
pub mod git_tracker;
pub mod shadow_git;
pub mod game_skill;
pub mod bench;
pub mod config;
pub mod layered_context_builder;

// Modules used by re-exports (not always declared as pub mod)
mod agent_comm;
mod scene_change;

// Re-export commonly used types
pub use agent::{BaseAgent, AgentInstanceId, AgentState, AgentConfig, AgentResult, StepResult};
pub use agent_platform::{
    AgentPlatform, AgentPlatformConfig, AgentPlatformError, AgentPlatformEvent,
    AgentPlatformMessage, AgentPlatformRole, AgentPlatformRunResult, AgentPlatformStatus,
    AgentRunId, AgentRunState, AgentSession, AgentSessionId, PendingToolApproval,
};
pub use bevy_editor_model::{
    BevyEditorCommand, ComponentOverride, ComponentPatch as EditorComponentPatch,
    ComponentPropertySchema, ComponentSchema, ComponentSchemaRegistry, ComponentValueType,
    LevelDocument, LevelId, PrefabDefinition, PrefabId, PrefabInstanceId,
    PrefabInstanceInfo, PrefabNode, PrefabRegistry,
};
pub use builtin_skills::{
    create_entity_skill, modify_entity_transform_skill, query_scene_skill,
    import_asset_skill, register_builtin_skills,
};
pub use code_tools::register_code_tools;
pub use file_tools::register_file_tools;
pub use director::{
    DirectorRuntime, EditorCommand, EditorEvent, DirectorTraceEntry,
    DirectorExecutionResult, ExecuteContext,
};
pub use engine_tools::{
    GetEngineStateTool, BuildProjectTool, PlayGameTool, ExportAssetTool,
    ReviewCodeTool, ApplyCodeChangeTool, register_engine_tools,
};
pub use goal::{GoalState, GoalRequirement, GoalRequirementKind, GoalCheckResult, GoalRequirementResult};
pub use index::{
    ProjectIndex, CrateEntry, AssetEntry, DocEntry,
    SemanticIndex, SemanticCategory,
    SkillIndex as IndexSkillIndex, SkillIndexEntry,
};
pub use llm::{
    LlmClient, LlmConfig, LlmProvider, LlmRequest, LlmResponse, LlmMessage, Role,
    LlmConfigSource, create_llm_client, config_from_env, check_api_keys,
};
pub use mcp_registry::{
    McpToolRegistry, McpToolDescriptor, ModelCompatibility,
};
pub use message_buffer::MessageBuffer;
pub use persistent_memory::{
    PersistentMemory, UserPreferences, ConfirmationLevel, LearnedPattern, EntityKnowledge,
};
pub use memory::{
    MemorySystem, MemoryConfig, MemoryQuery, MemoryContext, MemoryStats,
    MemoryTier, MemoryEntryId, MemoryMetadata,
    WorkingMemory, WorkingMemoryEntry, WorkingEntryType,
    EpisodicMemory, Episode, EpisodeType, EpisodeSearchResult,
    SemanticMemory, SemanticNode, SemanticRelation, RelationType,
    ProceduralMemory, WorkflowTemplate, WorkflowStep, DecisionPattern,
    HybridRetriever, RetrievalQuery, RetrievalResult, RetrievalStream,
    MemoryLifecycle, DecayConfig, MemoryImportance,
};
pub use metrics::{AgentMetrics, PerformanceTracer};
pub use permission::{OperationRisk, PermissionDecision, PermissionRequirement, PermissionEngine};
pub use plan::{EditPlan, EditPlanStep, ExecutionMode, EditPlanStatus, TargetModule, ExpectedChangeSet, ChangeKind};
pub use planner::{RuleBasedPlanner, LlmPlanner, Planner, PlannerContext, ComplexityLevel};
pub use hybrid_controller::{
    HybridEditorController, HybridLlmStatus, EditorMode, FallbackReason, FallbackEvent, HybridStats,
};
pub use prompt::{PromptSystem, PromptType, PromptTemplate, PromptContext, BASE_SYSTEM_PROMPT, BEVY_SPECIFIC_PROMPT, TokenBudget, TokenAllocation, estimate_tokens};
pub use project::{
    ProjectManifest, ProjectManager, ProjectTemplate, ProjectError,
    RecentProject, RecentProjectsList, AgentProjectConfig,
    is_valid_project_dir, find_project_root, PROJECT_MANIFEST_FILE,
};
pub use registry::{
    AgentRegistry, AgentId, CapabilityKind, AgentRole, SpecialistKind,
    Agent, AgentRequest, AgentResponse, AgentResultKind, AgentError,
};
pub use review::{ReviewSummary, ReviewerDecision, Reviewer};
pub use runtime_agent::{
    EditorAgentControlCommand, RuntimeAgentAction, RuntimeAgentComponent,
    RuntimeAgentControlMode, RuntimeAgentEvent, RuntimeAgentId, RuntimeAgentInstance,
    RuntimeAgentProfile, RuntimeAgentProfileId, RuntimeAgentRegistry, RuntimeAgentStatus,
    RuntimeAgentTickResult, RuntimeBehaviorSpec, RuntimeBehaviorState,
    RuntimeBehaviorTransition, RuntimeBlackboard, RuntimeCondition, RuntimeConsideration,
    RuntimeGoal, RuntimeMemoryPolicy, RuntimeObservation, RuntimeTarget,
    evaluate_runtime_agent_tick,
};
pub use runtime_agent_tools::{
    AttachRuntimeAgentTool, SetAgentControlModeTool, SetAgentGoalTool,
    SetAgentBlackboardTool, QueryRuntimeAgentsTool, register_runtime_agent_tools,
};
pub use rollback::{
    RollbackManager, OperationLog, OperationId, OperationType, Change,
    AssetAction, SceneSnapshot, SnapshotEntity,
};
pub use scene_agent::SceneAgent;
pub use specialized_agents::{CodeAgent, ReviewAgent, EditorAgent, PlannerAgent};
pub use scene_bridge::{
    SceneBridge, MockSceneBridge, EntityListItem, ComponentPatch,
    SharedSceneBridge, create_shared_bridge, create_empty_shared_bridge,
};
pub use scene_tools::register_scene_tools;
pub use skill::{
    SkillId, SkillDefinition, SkillNode, SkillEdge, SkillInput, SkillInputType,
    RetryPolicy, SkillEdgeCondition, SkillInstance, SkillInstanceStatus, NodeState,
    SkillExecutor, SkillRegistry,
};
pub use strategy::{ReActAgent, ReActConfig, ReActStep, ReActError, create_react_agent};
pub use task::TaskId;
pub use tool::{Tool, ToolRegistry, ToolCall, ToolResult, ToolCategory};
pub use transaction::{EditTransaction, TransactionStore, TransactionStatus, EditOperation, RollbackOperation};
pub use event::{EventBus, EventSource};
pub use fallback::{FallbackEngine, FallbackResult, CodeTemplate, TemplateLibrary, RuleEngine, Rule};
pub use types::*;
pub use vision::{
    VisualObservation, VisionContent, ImageUrl, VisionMessage,
    VisionRequest, VisionResponse, VisionUsage, VisionError, VisionModel,
    VisionClient, observation_to_content, create_vision_message,
};
pub use agent_comm::{
    AgentMessage, MessageType as AgentCommMessageType, MessagePayload, MessageId, TaskPriority,
    SharedContext, MessageBroker, CommunicationHub, AgentCommError,
};
pub use scene_change::{
    SceneChangeTracker, SceneChangeSummary, EntityChange, ChangeType,
    ComponentChangeSummary, ChangeDetectionStrategy, ChangeDetectionConfig,
    SceneChangeProvider,
};
pub use edit_ops::{
    EditOp, EditOpError,
    CreateEntityOp, DeleteEntityOp, EntitySnapshot,
    SetTransformOp, SetColorOp, SetVisibilityOp,
    MultiOp,
};
pub use event_stream::{
    AgentEvent, EventStreamBroker, AgentUiConsumer, EventReplay,
};
pub use edit_history::EditHistory;
pub use context_collector::RuntimeContextCollector;
pub use git_tracker::{
    GitTracker, GitConfig, GitError, CommitInfo, AgentActionType, GitRollbackBridge,
};
pub use game_skill::{
    GameSkill, TemplateSkill, DebugSkill, ProjectSkeleton, SkeletonFile,
    VerifiedFix, ErrorSignature, FixStrategy, VerificationMethod,
    FixResult, ScaffoldResult, GameEngine, SkillError,
};
pub use bench::{
    BenchRunner, BenchScore, BuildHealthScore, VisualUsabilityScore, IntentAlignmentScore,
    BuildHealthEvaluator, VisualUsabilityEvaluator, IntentAlignmentEvaluator,
    Evaluator, BenchError, BenchIntegration,
};
pub use config::{
    AgentEditConfig, ConfigLoader, ConfigError, AssetProvidersConfig,
    AgentBehaviorConfig, ApprovalMode, UiConfig, GitSettingsConfig,
    BenchSettingsConfig, GameSkillConfig, EXAMPLE_SETTINGS,
    init_global_config, get_config, get_config_ref,
};
pub use layered_context_builder::LayeredContextBuilder;
pub use dynamic_planner::DynamicPlanner;
pub use reflection_engine::ReflectionEngine;

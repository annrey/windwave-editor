//! Agent Core - Phase 1: BaseAgent Framework + State Machine + Memory System
//!
//! This crate defines the foundational Agent architecture for AgentEdit,
//! following the OpenClaw + OpenManus inspired design.

#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::result_large_err)]
#![allow(clippy::ptr_arg)]
#![allow(clippy::unnecessary_to_owned)]
#![allow(clippy::cloned_ref_to_slice_refs)]

pub mod agent;
pub mod agent_collaboration;
pub mod agent_context;
pub mod agent_pipeline;
pub mod agent_platform;
pub mod agent_snapshot;
pub mod ai_frameworks;
pub mod ai_resident;
#[cfg(feature = "http-api")]
pub mod api;
pub mod audit;
pub mod bench;
pub mod bevy_editor_model;
pub mod builtin_skills;
pub mod capture_pipeline;
pub mod ceo;
pub mod cli_adapter;
pub mod cli_agent;
pub mod cli_defs;
pub mod cli_stream;
pub mod code_graph;
pub mod code_tools;
pub mod command;
pub mod config;
pub mod context;
pub mod context_collector;
pub mod director;
pub mod dynamic_planner;
pub mod edit_history;
pub mod edit_ops;
pub mod engine_tools;
pub mod event;
pub mod event_stream;
pub mod fallback;
pub mod file_tools;
pub mod game_mode;
pub mod game_skill;
pub mod gameplay_primitive;
pub mod git_tracker;
pub mod goal;
pub mod goal_checker;
pub mod hr_agent;
pub mod hybrid_controller;
pub mod index;
pub mod keyword_matcher;
pub mod layered_context_builder;
pub mod lifecycle;
pub mod livelog;
pub mod llm;
pub mod mcp_registry;
pub mod memory;
pub mod memory_injector;
pub mod message_buffer;
pub mod metrics;
pub mod module;
pub mod narrative;
pub mod open_world_plan;
pub mod open_world_runtime;
pub mod open_world_template;
pub mod open_world_timeline;
pub mod open_world_verification;
pub mod open_world_visual_snapshot;
pub mod path_scanner;
pub mod permission;
pub mod persistent_memory;
pub mod plan;
pub mod planner;
pub mod playable_scenario;
pub mod project;
pub mod project_system;
pub mod prompt;
pub mod reasoning_bank;
pub mod reflection_engine;
pub mod registry;
pub mod review;
pub mod rollback;
pub mod router;
pub mod rule_engine;
pub mod rule_system;
pub mod runtime_agent;
pub mod runtime_agent_tools;
pub mod runtime_registry;
pub mod scene_agent;
pub mod scene_bridge;
pub mod scene_serializer;
pub mod scene_tools;
pub mod search_engine;
pub mod selection_tools;
pub mod shadow_git;
pub mod skill;
pub mod skills_compound;
pub mod specialized_agents;
pub mod squad;
pub mod squad_agent;
pub mod strategy;
pub mod task;
pub mod team_context;
pub mod team_structure;
pub mod tool;
pub mod transaction;
pub mod types;
pub mod vision;
pub mod visual_script;
pub mod visual_system;
pub mod world_clock;

// Modules used by re-exports (not always declared as pub mod)
mod agent_comm;
mod scene_change;

// Re-export commonly used types
pub use agent::{AgentConfig, AgentInstanceId, AgentResult, AgentState, BaseAgent, StepResult};
pub use agent_collaboration::{
    AgentCollaborationSystem, CollaborationExample, SquadBuilder, TaskRecommendations,
};
pub use agent_comm::{
    AgentCommError, AgentMessage, CommunicationHub, MessageBroker, MessageId, MessagePayload,
    MessageType as AgentCommMessageType, SharedContext,
};
pub use agent_platform::{
    AgentPlatform, AgentPlatformConfig, AgentPlatformError, AgentPlatformEvent,
    AgentPlatformMessage, AgentPlatformRole, AgentPlatformRunResult, AgentPlatformStatus,
    AgentRunId, AgentRunState, AgentSession, AgentSessionId, PendingToolApproval,
};
pub use ai_frameworks::AIFrameworkManager;
pub use ai_resident::{
    ActionTemplateId, AdjudicationOutcome, AiResidentError, AiResidentSandbox, ResidentIntent,
    ResidentObservation, ResidentProfile, ResidentRole, MAX_AI_RESIDENTS,
    SLICE_ID as AI_RESIDENT_SLICE_ID,
};
pub use bench::{
    BenchError, BenchIntegration, BenchRunner, BenchScore, BuildHealthEvaluator, BuildHealthScore,
    Evaluator, IntentAlignmentEvaluator, IntentAlignmentScore, VisualUsabilityEvaluator,
    VisualUsabilityScore,
};
pub use bevy_editor_model::{
    BevyEditorCommand, ComponentOverride, ComponentPatch as EditorComponentPatch,
    ComponentPropertySchema, ComponentSchema, ComponentSchemaRegistry, ComponentValueType,
    LevelDocument, LevelId, PrefabDefinition, PrefabId, PrefabInstanceId, PrefabInstanceInfo,
    PrefabNode, PrefabRegistry,
};
pub use builtin_skills::{
    create_entity_skill, import_asset_skill, modify_entity_transform_skill, query_scene_skill,
    register_builtin_skills,
};
pub use ceo::{
    CeoAgent, DirectorHandle, DirectorMetrics, DirectorRegistry, DirectorStatus,
    DirectorStatusReport, GoalConstraints, GoalId, GoalStatus, HighLevelGoal, ProjectManagerId,
    ResourceBudget,
};
pub use code_tools::register_code_tools;
pub use config::{
    get_config, get_config_ref, init_global_config, AgentBehaviorConfig, AgentEditConfig,
    ApprovalMode, AssetProvidersConfig, BenchSettingsConfig, ConfigError, ConfigLoader,
    GameSkillConfig, GitSettingsConfig, UiConfig, EXAMPLE_SETTINGS,
};
pub use context_collector::RuntimeContextCollector;
pub use director::{
    DirectorExecutionResult, DirectorRuntime, DirectorTraceEntry, EditorCommand, EditorEvent,
    ExecuteContext,
};
pub use dynamic_planner::DynamicPlanner;
pub use edit_history::EditHistory;
pub use edit_ops::{
    CreateEntityOp, DeleteEntityOp, EditOp, EditOpError, EntitySnapshot, MultiOp, SetColorOp,
    SetTransformOp, SetVisibilityOp,
};
pub use engine_tools::{
    register_engine_tools, ApplyCodeChangeTool, BuildProjectTool, ExportAssetTool,
    GetEngineStateTool, PlayGameTool, ReviewCodeTool,
};
pub use event::{EventBus, EventSource};
pub use event_stream::{AgentEvent, AgentUiConsumer, EventReplay, EventStreamBroker};
pub use fallback::{
    CodeTemplate, FallbackEngine, FallbackResult, Rule, RuleEngine, TemplateLibrary,
};
pub use file_tools::register_file_tools;
pub use game_skill::{
    DebugSkill, ErrorSignature, FixResult, FixStrategy, GameEngine, GameSkill, ProjectSkeleton,
    ScaffoldResult, SkeletonFile, SkillError, TemplateSkill, VerificationMethod, VerifiedFix,
};
pub use gameplay_primitive::{
    GameplayCapability, GameplayPrimitiveCatalog, GameplayPrimitiveCategory,
    GameplayPrimitiveDescriptor, GameplayPrimitiveKind, GameplayPrimitiveValidationError,
};
pub use git_tracker::{
    AgentActionType, CommitInfo, GitConfig, GitError, GitRollbackBridge, GitTracker,
};
pub use goal::{
    GoalCheckResult, GoalRequirement, GoalRequirementKind, GoalRequirementResult, GoalState,
};
pub use hybrid_controller::{
    BypassEntry, EditorMode, FallbackEvent, FallbackReason, Feature, FeatureLimiter,
    HybridEditorController, HybridLlmStatus, HybridStats, LimitReason,
};
pub use index::{
    AssetEntry, CrateEntry, DocEntry, ProjectIndex, SemanticCategory, SemanticIndex,
    SkillIndex as IndexSkillIndex, SkillIndexEntry,
};
pub use layered_context_builder::LayeredContextBuilder;
pub use livelog::{
    create_shared_livelog, Livelog, LivelogCallback, LivelogEntry, LivelogLevel, SharedLivelog,
};
pub use llm::{
    check_api_keys, config_from_env, create_llm_client, LlmClient, LlmConfig, LlmConfigSource,
    LlmMessage, LlmProvider, LlmRequest, LlmResponse, Role,
};
pub use mcp_registry::{McpToolDescriptor, McpToolRegistry, ModelCompatibility};
pub use memory::{
    AgentMemoryEntry, AgentMemoryId, DecayConfig, DecisionPattern, Episode, EpisodeSearchResult,
    EpisodeType, EpisodicMemory, HybridRetriever, MemoryConfig, MemoryContext, MemoryEntryId,
    MemoryImportance, MemoryLifecycle, MemoryMetadata, MemoryQuery, MemoryStats, MemorySystem,
    MemorySystemRegistry, MemoryTier, ProceduralMemory, RelationType, RetrievalQuery,
    RetrievalResult, RetrievalStream, SemanticMemory, SemanticNode, SemanticRelation, WorkflowStep,
    WorkflowTemplate, WorkingEntryType, WorkingMemory, WorkingMemoryEntry,
};
pub use message_buffer::MessageBuffer;
pub use metrics::{AgentMetrics, PerformanceTracer};
pub use open_world_plan::{
    OpenWorldObjectKind, OpenWorldObjectSpec, OpenWorldPlan, OpenWorldPlanRisk,
    OpenWorldPlanValidationError, OpenWorldPlanValidationProblem, OpenWorldTask,
    OpenWorldTaskGraph, OpenWorldTaskTarget, OpenWorldVerificationGoal, QuestFlowSpec,
    QuestObjectiveKind, QuestObjectiveSpec, QuestStateSpec, VerificationGoalKind, WorldConstraints,
    WorldLayout, WorldSpec, WorldZoneRole, WorldZoneSpec, ZoneLocation,
};
pub use open_world_runtime::{
    CombatFaction, CombatHitReport, CombatantRuntime, EnemyRuntimeState, LootRuntimeState,
    OpenWorldRuntimeError, OpenWorldRuntimeEvent, OpenWorldRuntimeObject, OpenWorldRuntimeState,
    PuzzleRuntimeState, QuestObjectiveRuntime, QuestObjectiveRuntimeState, QuestRuntimeState,
};
pub use open_world_template::{
    OpenWorldSceneTemplate, OpenWorldTemplateApplyReport, OpenWorldTemplateCreatedEntity,
    OpenWorldTemplateEntity,
};
pub use open_world_timeline::{
    OpenWorldReplayWorldState, OpenWorldTimeline, OpenWorldTimelineTick,
};
pub use open_world_verification::{
    OpenWorldVerificationBundle, OpenWorldVerificationResult, PlaytestVerificationSummary,
    VerificationBundleStatus, VerificationEvidence, VerificationGoalStatus,
};
pub use permission::{OperationRisk, PermissionDecision, PermissionEngine, PermissionRequirement};
pub use persistent_memory::{
    ConfirmationLevel, EntityKnowledge, LearnedPattern, PersistentMemory, UserPreferences,
};
pub use plan::{
    ChangeKind, EditPlan, EditPlanStatus, EditPlanStep, ExecutionMode, ExpectedChangeSet,
    TargetModule,
};
pub use planner::{ComplexityLevel, LlmPlanner, Planner, PlannerContext, RuleBasedPlanner};
pub use playable_scenario::{
    PlayableObjectState, PlayableScenario, PlayableScenarioFailureReport, PlayableScenarioReport,
    PlayableScenarioState, PlayableScenarioStep, PlayableScenarioTimeEvidence,
};
pub use project::{
    find_project_root, is_valid_project_dir, AgentProjectConfig, ProjectError, ProjectManager,
    ProjectManifest, ProjectTemplate, RecentProject, RecentProjectsList, PROJECT_MANIFEST_FILE,
};
pub use prompt::{
    estimate_tokens, PromptContext, PromptSystem, PromptTemplate, PromptType, TokenAllocation,
    TokenBudget, BASE_SYSTEM_PROMPT, BEVY_SPECIFIC_PROMPT,
};
pub use reasoning_bank::{
    ReasoningBank, ReasoningBankManager, ReasoningPattern, ReasoningStep, ReasoningStepId,
    ReasoningTrace, ReasoningTraceId, StepType,
};
pub use reflection_engine::ReflectionEngine;
pub use registry::{
    Agent, AgentError, AgentId, AgentRegistry, AgentRequest, AgentResponse, AgentResultKind,
    AgentRole, CapabilityKind, SpecialistKind,
};
pub use review::{ReviewSummary, Reviewer, ReviewerDecision};
pub use rollback::{
    AssetAction, Change, OperationId, OperationLog, OperationType, RollbackManager, SceneSnapshot,
    SnapshotEntity,
};
pub use runtime_agent::{
    evaluate_runtime_agent_tick, EditorAgentControlCommand, RuntimeAgentAction,
    RuntimeAgentComponent, RuntimeAgentControlMode, RuntimeAgentEvent, RuntimeAgentId,
    RuntimeAgentInstance, RuntimeAgentProfile, RuntimeAgentProfileId, RuntimeAgentRegistry,
    RuntimeAgentStatus, RuntimeAgentTickResult, RuntimeBehaviorSpec, RuntimeBehaviorState,
    RuntimeBehaviorTransition, RuntimeBlackboard, RuntimeCondition, RuntimeConsideration,
    RuntimeGoal, RuntimeMemoryPolicy, RuntimeObservation, RuntimeTarget,
};
pub use runtime_agent_tools::{
    register_runtime_agent_tools, AttachRuntimeAgentTool, QueryRuntimeAgentsTool,
    SetAgentBlackboardTool, SetAgentControlModeTool, SetAgentGoalTool,
};
pub use runtime_registry::{
    EngineType, RuntimeCapability, RuntimeConfig, RuntimeId, RuntimeInfo, RuntimeManager,
    RuntimeRegistry,
};
pub use scene_agent::SceneAgent;
pub use scene_bridge::{
    create_empty_shared_bridge, create_shared_bridge, ComponentPatch, EntityListItem,
    MockSceneBridge, SceneBridge, SharedSceneBridge,
};
pub use scene_change::{
    ChangeDetectionConfig, ChangeDetectionStrategy, ChangeType, ComponentChangeSummary,
    EntityChange, SceneChangeProvider, SceneChangeSummary, SceneChangeTracker,
};
pub use scene_tools::register_scene_tools;
pub use skill::{
    NodeState, RetryPolicy, SkillDefinition, SkillEdge, SkillEdgeCondition, SkillExecutor, SkillId,
    SkillInput, SkillInputType, SkillInstance, SkillInstanceStatus, SkillNode, SkillRegistry,
};
pub use skills_compound::{
    CompoundSkill, CompoundSkillId, PatternStep, SkillCompoundManager, SkillCompoundRegistry,
    TaskPattern,
};
pub use specialized_agents::{CodeAgent, EditorAgent, PlannerAgent, ReviewAgent};
pub use squad::{RoutingPolicy, Squad, SquadId, SquadRegistry, SquadTask, TaskRouter, TaskStatus};
pub use squad_agent::SquadAgent;
pub use strategy::{create_react_agent, ReActAgent, ReActConfig, ReActError, ReActStep};
pub use task::TaskId;
pub use tool::{Tool, ToolCall, ToolCategory, ToolRegistry, ToolResult};
pub use transaction::{
    EditOperation, EditTransaction, RollbackOperation, TransactionStatus, TransactionStore,
};
pub use types::*;
pub use vision::{
    create_vision_message, observation_to_content, ImageUrl, VisionClient, VisionContent,
    VisionError, VisionMessage, VisionModel, VisionRequest, VisionResponse, VisionUsage,
    VisualObservation,
};
pub use world_clock::{
    AgentSchedule, CalendarEvent, CalendarEventState, OfflineProgressionPolicy,
    OfflineProgressionReport, ReplayLedger, ReplayLedgerEntry, ScheduleDecision, ScheduleWindow,
    TimePolicy, WorldClock, WorldClockError, WorldClockMode, WorldTimestamp,
};

// New CLI agent system (multica/Open Design inspired)
pub use cli_adapter::{CliAdapterError, CliAgentPool, CliAgentStream, CliDetectedAgent};
pub use cli_defs::{
    agent_defs_by_protocol, agent_defs_by_tag, find_agent_def, types::AgentDef, AIDER_AGENT_DEF,
    ALL_AGENT_DEFS, CLAUDE_AGENT_DEF, CODEX_AGENT_DEF, COPILOT_AGENT_DEF, CURSOR_AGENT_DEF,
    DEEPSEEK_AGENT_DEF, DEVIN_AGENT_DEF, GEMINI_AGENT_DEF, HERMES_AGENT_DEF, KILO_AGENT_DEF,
    KIMI_AGENT_DEF, KIRO_AGENT_DEF, OPENCODE_AGENT_DEF, PI_AGENT_DEF, QODER_AGENT_DEF,
    QWEN_AGENT_DEF, VIBE_AGENT_DEF,
};
pub use cli_stream::{
    parser_for_protocol, AcpJsonRpcParser, CliArgBuilder, CliCapabilities, CliEvent,
    CliOutputParser, CliProtocol, PlainTextParser, PromptInputFormat, StreamJsonParser, TodoItem,
    TodoStatus, UsageInfo, UserQuestionOption,
};
pub use path_scanner::{detect_agent, scan_all_agents, toolchain_bins, CliDetection};

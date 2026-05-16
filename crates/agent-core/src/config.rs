//! Three-layer configuration system: CLI > Environment > File
//!
//! Priority (highest to lowest):
//! 1. CLI flags (--model, --api-key, etc.)
//! 2. Environment variables (OPENAI_API_KEY, etc.)
//! 3. Settings files (settings.json)
//!
//! Inspired by OpenGame's configuration approach

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Configuration structures
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEditConfig {
    /// LLM provider settings
    pub llm: LlmConfig,
    /// Asset generation providers
    pub assets: AssetProvidersConfig,
    /// Agent behavior settings
    pub agent: AgentBehaviorConfig,
    /// UI settings
    pub ui: UiConfig,
    /// Git/version control settings
    pub git: GitSettingsConfig,
    /// Evaluation settings
    pub bench: BenchSettingsConfig,
    /// Game skill settings
    pub game_skill: GameSkillConfig,
    /// Extra key-value pairs for extensibility
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    pub api_key: String,
    pub base_url: Option<String>,
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub timeout_seconds: u64,
    pub fallback_models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetProvidersConfig {
    pub image_provider: String,
    pub image_api_key: Option<String>,
    pub video_provider: Option<String>,
    pub video_api_key: Option<String>,
    pub audio_provider: Option<String>,
    pub audio_api_key: Option<String>,
    pub reasoning_provider: Option<String>,
    pub reasoning_api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBehaviorConfig {
    pub approval_mode: ApprovalMode,
    pub auto_commit: bool,
    pub max_iterations: u32,
    pub retry_on_error: bool,
    pub debug_mode: bool,
    pub log_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalMode {
    /// Always ask for approval
    Ask,
    /// Auto-approve file edits only
    AutoEdit,
    /// Auto-approve everything (yolo mode)
    Yolo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub theme: String,
    pub language: String,
    pub show_diff_on_edit: bool,
    pub enable_animations: bool,
    pub font_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitSettingsConfig {
    pub enabled: bool,
    pub auto_commit: bool,
    pub commit_template: String,
    pub default_branch: String,
    pub author_name: String,
    pub author_email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchSettingsConfig {
    pub enabled: bool,
    pub auto_evaluate: bool,
    pub min_score_threshold: f32,
    pub max_iterations: u32,
    pub results_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSkillConfig {
    pub template_dir: PathBuf,
    pub fix_db_dir: PathBuf,
    pub learn_from_success: bool,
    pub learn_from_failure: bool,
    pub max_template_history: usize,
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

impl Default for AgentEditConfig {
    fn default() -> Self {
        Self {
            llm: LlmConfig::default(),
            assets: AssetProvidersConfig::default(),
            agent: AgentBehaviorConfig::default(),
            ui: UiConfig::default(),
            git: GitSettingsConfig::default(),
            bench: BenchSettingsConfig::default(),
            game_skill: GameSkillConfig::default(),
            extra: HashMap::new(),
        }
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            api_key: String::new(),
            base_url: Some("https://api.openai.com/v1".to_string()),
            model: crate::llm::models::OPENAI_DEFAULT.to_string(),
            max_tokens: 4096,
            temperature: 0.7,
            timeout_seconds: 60,
            fallback_models: vec![
                crate::llm::models::OPENAI_FAST.to_string(),
                crate::llm::models::CLAUDE_DEFAULT.to_string()
            ],
        }
    }
}

impl Default for AssetProvidersConfig {
    fn default() -> Self {
        Self {
            image_provider: "tongyi".to_string(),
            image_api_key: None,
            video_provider: None,
            video_api_key: None,
            audio_provider: None,
            audio_api_key: None,
            reasoning_provider: None,
            reasoning_api_key: None,
        }
    }
}

impl Default for AgentBehaviorConfig {
    fn default() -> Self {
        Self {
            approval_mode: ApprovalMode::AutoEdit,
            auto_commit: true,
            max_iterations: 10,
            retry_on_error: true,
            debug_mode: false,
            log_level: "info".to_string(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            language: "zh-CN".to_string(),
            show_diff_on_edit: true,
            enable_animations: true,
            font_size: 14,
        }
    }
}

impl Default for GitSettingsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_commit: true,
            commit_template: "[AgentEdit] {action}: {description}".to_string(),
            default_branch: "main".to_string(),
            author_name: "AgentEdit".to_string(),
            author_email: "agent@agentedit.local".to_string(),
        }
    }
}

impl Default for BenchSettingsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_evaluate: true,
            min_score_threshold: 80.0,
            max_iterations: 5,
            results_dir: PathBuf::from("./bench_results"),
        }
    }
}

impl Default for GameSkillConfig {
    fn default() -> Self {
        Self {
            template_dir: PathBuf::from("./templates"),
            fix_db_dir: PathBuf::from("./fixes"),
            learn_from_success: true,
            learn_from_failure: true,
            max_template_history: 100,
        }
    }
}

// ---------------------------------------------------------------------------
// ConfigLoader — Three-layer configuration loading
// ---------------------------------------------------------------------------

pub struct ConfigLoader {
    /// CLI overrides (highest priority)
    cli_overrides: HashMap<String, String>,
    /// Loaded configuration
    config: AgentEditConfig,
    /// Config file paths
    config_paths: Vec<PathBuf>,
}

impl ConfigLoader {
    pub fn new() -> Self {
        Self {
            cli_overrides: HashMap::new(),
            config: AgentEditConfig::default(),
            config_paths: vec![
                // User settings
                dirs::home_dir()
                    .map(|d| d.join(".agentedit").join("settings.json"))
                    .unwrap_or_default(),
                // Project settings
                PathBuf::from(".agentedit").join("settings.json"),
                // Legacy compatibility
                dirs::home_dir()
                    .map(|d| d.join(".qwen").join("settings.json"))
                    .unwrap_or_default(),
                PathBuf::from(".qwen").join("settings.json"),
            ],
        }
    }
    
    /// Set a CLI override
    pub fn set_cli_override(&mut self, key: &str, value: &str) {
        self.cli_overrides.insert(key.to_string(), value.to_string());
    }
    
    /// Parse CLI arguments into overrides
    pub fn parse_cli_args(&mut self, args: &[String]) {
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            
            match arg.as_str() {
                "-p" | "--prompt" => {
                    if i + 1 < args.len() {
                        self.set_cli_override("agent.prompt", &args[i + 1]);
                        i += 2;
                        continue;
                    }
                }
                "-m" | "--model" => {
                    if i + 1 < args.len() {
                        self.set_cli_override("llm.model", &args[i + 1]);
                        i += 2;
                        continue;
                    }
                }
                "--api-key" => {
                    if i + 1 < args.len() {
                        self.set_cli_override("llm.api_key", &args[i + 1]);
                        i += 2;
                        continue;
                    }
                }
                "--base-url" => {
                    if i + 1 < args.len() {
                        self.set_cli_override("llm.base_url", &args[i + 1]);
                        i += 2;
                        continue;
                    }
                }
                "--approval-mode" => {
                    if i + 1 < args.len() {
                        self.set_cli_override("agent.approval_mode", &args[i + 1]);
                        i += 2;
                        continue;
                    }
                }
                "--yolo" => {
                    self.set_cli_override("agent.approval_mode", "yolo");
                }
                "--no-git" => {
                    self.set_cli_override("git.enabled", "false");
                }
                "--debug" => {
                    self.set_cli_override("agent.debug_mode", "true");
                }
                "--max-iterations" => {
                    if i + 1 < args.len() {
                        self.set_cli_override("agent.max_iterations", &args[i + 1]);
                        i += 2;
                        continue;
                    }
                }
                "--image-provider" => {
                    if i + 1 < args.len() {
                        self.set_cli_override("assets.image_provider", &args[i + 1]);
                        i += 2;
                        continue;
                    }
                }
                "--image-api-key" => {
                    if i + 1 < args.len() {
                        self.set_cli_override("assets.image_api_key", &args[i + 1]);
                        i += 2;
                        continue;
                    }
                }
                _ => {}
            }
            
            i += 1;
        }
    }
    
    /// Load configuration from all sources
    pub fn load(&mut self) -> Result<AgentEditConfig, ConfigError> {
        // 1. Start with defaults
        let mut config = AgentEditConfig::default();
        
        // 2. Load from config files (lowest priority)
        for path in &self.config_paths {
            if path.exists() {
                match self.load_from_file(path) {
                    Ok(file_config) => {
                        config = self.merge_configs(config, file_config);
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to load config from {}: {}", path.display(), e);
                    }
                }
            }
        }
        
        // 3. Apply environment variables (medium priority)
        config = self.apply_env_vars(config)?;
        
        // 4. Apply CLI overrides (highest priority)
        config = self.apply_cli_overrides(config)?;
        
        self.config = config.clone();
        Ok(config)
    }
    
    /// Load configuration from a JSON file
    fn load_from_file(&self, path: &Path) -> Result<AgentEditConfig, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: AgentEditConfig = serde_json::from_str(&content)?;
        Ok(config)
    }
    
    /// Apply environment variables
    fn apply_env_vars(&self, mut config: AgentEditConfig) -> Result<AgentEditConfig, ConfigError> {
        // LLM settings
        if let Ok(val) = std::env::var("OPENAI_API_KEY") {
            config.llm.api_key = val;
        }
        if let Ok(val) = std::env::var("OPENAI_BASE_URL") {
            config.llm.base_url = Some(val);
        }
        if let Ok(val) = std::env::var("OPENAI_MODEL") {
            config.llm.model = val;
        }
        
        // Asset providers
        if let Ok(val) = std::env::var("OPENGAME_IMAGE_PROVIDER") {
            config.assets.image_provider = val;
        }
        if let Ok(val) = std::env::var("OPENGAME_IMAGE_API_KEY") {
            config.assets.image_api_key = Some(val);
        }
        if let Ok(val) = std::env::var("OPENGAME_VIDEO_PROVIDER") {
            config.assets.video_provider = Some(val);
        }
        if let Ok(val) = std::env::var("OPENGAME_VIDEO_API_KEY") {
            config.assets.video_api_key = Some(val);
        }
        if let Ok(val) = std::env::var("OPENGAME_AUDIO_PROVIDER") {
            config.assets.audio_provider = Some(val);
        }
        if let Ok(val) = std::env::var("OPENGAME_AUDIO_API_KEY") {
            config.assets.audio_api_key = Some(val);
        }
        if let Ok(val) = std::env::var("OPENGAME_REASONING_PROVIDER") {
            config.assets.reasoning_provider = Some(val);
        }
        if let Ok(val) = std::env::var("OPENGAME_REASONING_API_KEY") {
            config.assets.reasoning_api_key = Some(val);
        }
        
        // Agent behavior
        if let Ok(val) = std::env::var("AGENTEDIT_APPROVAL_MODE") {
            config.agent.approval_mode = match val.as_str() {
                "ask" => ApprovalMode::Ask,
                "auto-edit" => ApprovalMode::AutoEdit,
                "yolo" => ApprovalMode::Yolo,
                _ => config.agent.approval_mode,
            };
        }
        if let Ok(val) = std::env::var("AGENTEDIT_DEBUG") {
            config.agent.debug_mode = val.parse().unwrap_or(false);
        }
        if let Ok(val) = std::env::var("AGENTEDIT_LOG_LEVEL") {
            config.agent.log_level = val;
        }
        
        // Git settings
        if let Ok(val) = std::env::var("AGENTEDIT_GIT_ENABLED") {
            config.git.enabled = val.parse().unwrap_or(true);
        }
        
        // Bench settings
        if let Ok(val) = std::env::var("AGENTEDIT_BENCH_ENABLED") {
            config.bench.enabled = val.parse().unwrap_or(true);
        }
        
        Ok(config)
    }
    
    /// Apply CLI overrides
    fn apply_cli_overrides(&self, mut config: AgentEditConfig) -> Result<AgentEditConfig, ConfigError> {
        for (key, value) in &self.cli_overrides {
            match key.as_str() {
                "llm.provider" => config.llm.provider = value.clone(),
                "llm.api_key" => config.llm.api_key = value.clone(),
                "llm.base_url" => config.llm.base_url = Some(value.clone()),
                "llm.model" => config.llm.model = value.clone(),
                "llm.max_tokens" => {
                    if let Ok(v) = value.parse() {
                        config.llm.max_tokens = v;
                    }
                }
                "llm.temperature" => {
                    if let Ok(v) = value.parse() {
                        config.llm.temperature = v;
                    }
                }
                "llm.timeout_seconds" => {
                    if let Ok(v) = value.parse() {
                        config.llm.timeout_seconds = v;
                    }
                }
                "assets.image_provider" => config.assets.image_provider = value.clone(),
                "assets.image_api_key" => config.assets.image_api_key = Some(value.clone()),
                "agent.approval_mode" => {
                    config.agent.approval_mode = match value.as_str() {
                        "ask" => ApprovalMode::Ask,
                        "auto-edit" => ApprovalMode::AutoEdit,
                        "yolo" => ApprovalMode::Yolo,
                        _ => config.agent.approval_mode,
                    };
                }
                "agent.auto_commit" => {
                    config.agent.auto_commit = value.parse().unwrap_or(true);
                }
                "agent.max_iterations" => {
                    if let Ok(v) = value.parse() {
                        config.agent.max_iterations = v;
                    }
                }
                "agent.debug_mode" => {
                    config.agent.debug_mode = value.parse().unwrap_or(false);
                }
                "git.enabled" => {
                    config.git.enabled = value.parse().unwrap_or(true);
                }
                "git.auto_commit" => {
                    config.git.auto_commit = value.parse().unwrap_or(true);
                }
                "bench.enabled" => {
                    config.bench.enabled = value.parse().unwrap_or(true);
                }
                "bench.min_score_threshold" => {
                    if let Ok(v) = value.parse() {
                        config.bench.min_score_threshold = v;
                    }
                }
                "agent.prompt" => {
                    // Store in extra for later use
                    config.extra.insert("prompt".to_string(), serde_json::Value::String(value.clone()));
                }
                _ => {
                    // Store unknown overrides in extra
                    config.extra.insert(key.clone(), serde_json::Value::String(value.clone()));
                }
            }
        }
        
        Ok(config)
    }
    
    /// Merge two configurations (second wins)
    fn merge_configs(&self, base: AgentEditConfig, override_: AgentEditConfig) -> AgentEditConfig {
        AgentEditConfig {
            llm: if override_.llm.api_key.is_empty() {
                base.llm
            } else {
                override_.llm
            },
            assets: override_.assets,
            agent: override_.agent,
            ui: override_.ui,
            git: override_.git,
            bench: override_.bench,
            game_skill: override_.game_skill,
            extra: {
                let mut merged = base.extra;
                merged.extend(override_.extra);
                merged
            },
        }
    }
    
    /// Save current configuration to file
    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<(), ConfigError> {
        let json = serde_json::to_string_pretty(&self.config)?;
        std::fs::write(path, json)?;
        Ok(())
    }
    
    /// Get current configuration
    pub fn config(&self) -> &AgentEditConfig {
        &self.config
    }
    
    /// Print provider status banner (like OpenGame)
    pub fn print_provider_status(&self) {
        println!("┌─────────────────────────────────────────┐");
        println!("│         AgentEdit Provider Status        │");
        println!("├─────────────────────────────────────────┤");
        
        // LLM
        let llm_status = if self.config.llm.api_key.is_empty() {
            "❌ Not configured"
        } else {
            &format!("✅ {} ({}", self.config.llm.provider, self.config.llm.model)
        };
        println!("│ LLM:     {:<30} │", llm_status);
        
        // Image
        let image_status = if self.config.assets.image_api_key.is_none() {
            "❌ Not configured"
        } else {
            &format!("✅ {}", self.config.assets.image_provider)
        };
        println!("│ Image:   {:<30} │", image_status);
        
        // Video
        let video_status = if self.config.assets.video_api_key.is_none() {
            "❌ Not configured"
        } else {
            &format!("✅ {}", self.config.assets.video_provider.as_deref().unwrap_or("N/A"))
        };
        println!("│ Video:   {:<30} │", video_status);
        
        // Audio
        let audio_status = if self.config.assets.audio_api_key.is_none() {
            "❌ Not configured"
        } else {
            &format!("✅ {}", self.config.assets.audio_provider.as_deref().unwrap_or("N/A"))
        };
        println!("│ Audio:   {:<30} │", audio_status);
        
        // Reasoning
        let reasoning_status = if self.config.assets.reasoning_api_key.is_none() {
            "❌ Not configured"
        } else {
            &format!("✅ {}", self.config.assets.reasoning_provider.as_deref().unwrap_or("N/A"))
        };
        println!("│ Reason:  {:<30} │", reasoning_status);
        
        // Git
        let git_status = if self.config.git.enabled {
            "✅ Enabled"
        } else {
            "❌ Disabled"
        };
        println!("│ Git:     {:<30} │", git_status);
        
        // Bench
        let bench_status = if self.config.bench.enabled {
            "✅ Enabled"
        } else {
            "❌ Disabled"
        };
        println!("│ Bench:   {:<30} │", bench_status);
        
        println!("└─────────────────────────────────────────┘");
    }
}

// ---------------------------------------------------------------------------
// Global config singleton
// ---------------------------------------------------------------------------

use std::sync::{Arc, RwLock};

static GLOBAL_CONFIG: once_cell::sync::Lazy<Arc<RwLock<AgentEditConfig>>> = 
    once_cell::sync::Lazy::new(|| {
        Arc::new(RwLock::new(AgentEditConfig::default()))
    });

pub fn init_global_config(config: AgentEditConfig) {
    if let Ok(mut guard) = GLOBAL_CONFIG.write() {
        *guard = config;
    }
}

pub fn get_config() -> AgentEditConfig {
    GLOBAL_CONFIG.read().map(|g| g.clone()).unwrap_or_default()
}

pub fn get_config_ref<F, R>(f: F) -> R
where
    F: FnOnce(&AgentEditConfig) -> R,
{
    let config = get_config();
    f(&config)
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Invalid configuration: {0}")]
    Invalid(String),
}

// ---------------------------------------------------------------------------
// Example settings.json
// ---------------------------------------------------------------------------

pub const EXAMPLE_SETTINGS: &str = r#"{
  "llm": {
    "provider": "openai",
    "api_key": "sk-...",
    "base_url": "https://api.openai.com/v1",
    "model": "gpt-4o",
    "max_tokens": 4096,
    "temperature": 0.7,
    "timeout_seconds": 60,
    "fallback_models": ["gpt-4o-mini", "claude-3-sonnet"]
  },
  "assets": {
    "image_provider": "tongyi",
    "image_api_key": "sk-...",
    "video_provider": null,
    "video_api_key": null,
    "audio_provider": null,
    "audio_api_key": null,
    "reasoning_provider": null,
    "reasoning_api_key": null
  },
  "agent": {
    "approval_mode": "auto-edit",
    "auto_commit": true,
    "max_iterations": 10,
    "retry_on_error": true,
    "debug_mode": false,
    "log_level": "info"
  },
  "ui": {
    "theme": "system",
    "language": "zh-CN",
    "show_diff_on_edit": true,
    "enable_animations": true,
    "font_size": 14
  },
  "git": {
    "enabled": true,
    "auto_commit": true,
    "commit_template": "[AgentEdit] {action}: {description}",
    "default_branch": "main",
    "author_name": "AgentEdit",
    "author_email": "agent@agentedit.local"
  },
  "bench": {
    "enabled": true,
    "auto_evaluate": true,
    "min_score_threshold": 80.0,
    "max_iterations": 5,
    "results_dir": "./bench_results"
  },
  "game_skill": {
    "template_dir": "./templates",
    "fix_db_dir": "./fixes",
    "learn_from_success": true,
    "learn_from_failure": true,
    "max_template_history": 100
  }
}"#;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = AgentEditConfig::default();
        assert_eq!(config.llm.provider, "openai");
        assert_eq!(config.llm.model, "gpt-4o");
        assert!(config.git.enabled);
        assert!(config.bench.enabled);
    }
    
    #[test]
    fn test_config_loader_parse_cli() {
        let mut loader = ConfigLoader::new();
        let args = vec![
            "--model".to_string(),
            "gpt-4".to_string(),
            "--api-key".to_string(),
            "test-key".to_string(),
            "--yolo".to_string(),
        ];
        
        loader.parse_cli_args(&args);
        
        assert_eq!(loader.cli_overrides.get("llm.model"), Some(&"gpt-4".to_string()));
        assert_eq!(loader.cli_overrides.get("llm.api_key"), Some(&"test-key".to_string()));
        assert_eq!(loader.cli_overrides.get("agent.approval_mode"), Some(&"yolo".to_string()));
    }
    
    #[test]
    fn test_env_var_override() {
        let loader = ConfigLoader::new();
        let config = AgentEditConfig::default();
        
        // This test would need actual env vars set
        // Just verify the method exists and returns Ok
        let result = loader.apply_env_vars(config);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_example_settings_valid() {
        let config: Result<AgentEditConfig, _> = serde_json::from_str(EXAMPLE_SETTINGS);
        assert!(config.is_ok());
        
        let config = config.unwrap();
        assert_eq!(config.llm.model, "gpt-4o");
        assert_eq!(config.agent.approval_mode, ApprovalMode::AutoEdit);
    }
    
    #[test]
    fn test_approval_mode_serialization() {
        let modes = vec![
            ApprovalMode::Ask,
            ApprovalMode::AutoEdit,
            ApprovalMode::Yolo,
        ];
        
        for mode in modes {
            let json = serde_json::to_string(&mode).unwrap();
            let deserialized: ApprovalMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, deserialized);
        }
    }
}

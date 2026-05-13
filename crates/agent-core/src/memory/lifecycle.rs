//! Memory Lifecycle - Decay, archiving, and cleanup
//!
//! Inspired by agentmemory's Ebbinghaus-inspired decay:
//! - Memories lose importance over time
//! - Access refreshes memory (spaced repetition)
//! - Low-importance memories are archived or deleted

use crate::memory::MemoryTier;
use serde::{Deserialize, Serialize};

/// Importance level of a memory
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MemoryImportance {
    Critical,    // Never decay (user preferences, critical errors)
    High,        // Slow decay
    Medium,      // Normal decay
    Low,         // Fast decay
    Transient,   // Very fast decay (temporary computations)
}

impl MemoryImportance {
    pub fn decay_rate(&self) -> f32 {
        match self {
            MemoryImportance::Critical => 0.0,
            MemoryImportance::High => 0.01,
            MemoryImportance::Medium => 0.05,
            MemoryImportance::Low => 0.15,
            MemoryImportance::Transient => 0.5,
        }
    }

    pub fn from_score(score: f32) -> Self {
        if score >= 0.9 { MemoryImportance::Critical }
        else if score >= 0.7 { MemoryImportance::High }
        else if score >= 0.4 { MemoryImportance::Medium }
        else if score >= 0.2 { MemoryImportance::Low }
        else { MemoryImportance::Transient }
    }
}

/// Configuration for memory lifecycle management
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DecayConfig {
    /// Half-life in seconds for medium-importance memories
    pub half_life_seconds: u64,
    /// Minimum importance before archiving
    pub archive_threshold: f32,
    /// Minimum importance before deletion
    pub delete_threshold: f32,
    /// Maximum number of episodic memories
    pub max_episodic: usize,
    /// Maximum number of semantic nodes
    pub max_semantic: usize,
    /// Maximum number of procedural workflows
    pub max_procedural: usize,
    /// Enable automatic cleanup
    pub auto_cleanup: bool,
}

impl Default for DecayConfig {
    fn default() -> Self {
        Self {
            half_life_seconds: 86400, // 1 day
            archive_threshold: 0.2,
            delete_threshold: 0.05,
            max_episodic: 1000,
            max_semantic: 500,
            max_procedural: 200,
            auto_cleanup: true,
        }
    }
}

/// Memory lifecycle manager
#[derive(Debug, Clone)]
pub struct MemoryLifecycle {
    pub config: DecayConfig,
}

impl MemoryLifecycle {
    pub fn new() -> Self {
        Self {
            config: DecayConfig::default(),
        }
    }

    pub fn with_config(config: DecayConfig) -> Self {
        Self { config }
    }

    /// Calculate current importance after decay
    pub fn current_importance(
        &self,
        initial_importance: f32,
        last_accessed: u64,
        access_count: u32,
        importance_level: MemoryImportance,
    ) -> f32 {
        let now = crate::types::current_timestamp() as f32;
        let age = now - last_accessed as f32;
        let half_life = self.config.half_life_seconds as f32;
        let decay_rate = importance_level.decay_rate();

        // Base decay: exponential based on age
        let time_decay = (-decay_rate * age / half_life).exp();

        // Access boost: more accesses = slower decay
        let access_boost = 1.0 + (access_count as f32).ln_1p() * 0.1;

        // Combined importance
        let importance = initial_importance * time_decay * access_boost;

        importance.clamp(0.0, 1.0)
    }

    /// Check if a memory should be archived
    pub fn should_archive(&self, importance: f32, tier: MemoryTier) -> bool {
        if !self.config.auto_cleanup {
            return false;
        }
        match tier {
            MemoryTier::Working => false, // Working memory is managed separately
            _ => importance < self.config.archive_threshold,
        }
    }

    /// Check if a memory should be deleted
    pub fn should_delete(&self, importance: f32, tier: MemoryTier) -> bool {
        if !self.config.auto_cleanup {
            return false;
        }
        match tier {
            MemoryTier::Working => false,
            _ => importance < self.config.delete_threshold,
        }
    }

    /// Check if a tier is over capacity
    pub fn is_over_capacity(&self, tier: MemoryTier, count: usize) -> bool {
        let max = match tier {
            MemoryTier::Episodic => self.config.max_episodic,
            MemoryTier::Semantic => self.config.max_semantic,
            MemoryTier::Procedural => self.config.max_procedural,
            MemoryTier::Working => usize::MAX, // Working memory has its own limit
        };
        count > max
    }

    /// Calculate how many entries to remove to get under capacity
    pub fn excess_count(&self, tier: MemoryTier, count: usize) -> usize {
        if !self.is_over_capacity(tier, count) {
            return 0;
        }
        let max = match tier {
            MemoryTier::Episodic => self.config.max_episodic,
            MemoryTier::Semantic => self.config.max_semantic,
            MemoryTier::Procedural => self.config.max_procedural,
            MemoryTier::Working => return 0,
        };
        count.saturating_sub(max)
    }
}

impl Default for MemoryLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

//! FeatureLimiter — LLM-dependent feature gating framework
//!
//! When the HybridEditorController falls back to RuleBased mode,
//! certain LLM-dependent features are disabled. FeatureLimiter
//! manages which features are available and provides a temporary
//! bypass mechanism with audit logging.

use super::EditorMode;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Feature that may be limited in degraded mode
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Feature {
    /// LLM-driven complex reasoning (multi-step planning)
    ComplexReasoning,
    /// Free-form natural language editing
    FreeFormEditing,
    /// AI-assisted code generation
    CodeGeneration,
    /// Contextual suggestions (memory-based)
    ContextualSuggestions,
    /// Visual understanding (VGRC)
    VisualUnderstanding,
    /// Procedural memory learning
    ProceduralLearning,
}

/// Reason a feature is limited
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LimitReason {
    /// Feature requires LLM
    RequiresLlm,
    /// Feature explicitly disabled by user
    UserDisabled,
}

/// Entry in the bypass audit log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BypassEntry {
    pub feature: Feature,
    pub timestamp: u64,
    pub user_confirmed: bool,
}

/// FeatureLimiter — controls feature availability based on editor mode
pub struct FeatureLimiter {
    /// Features that require LLM mode
    llm_required: HashSet<Feature>,
    /// Features explicitly disabled
    disabled: HashSet<Feature>,
    /// Temporarily bypassed features (feature -> expiry timestamp)
    bypassed: HashMap<Feature, u64>,
    /// Bypass audit log
    audit_log: Vec<BypassEntry>,
    /// Bypass duration in seconds
    bypass_duration: u64,
}

impl FeatureLimiter {
    /// Create a new FeatureLimiter with default LLM-required features
    pub fn new() -> Self {
        let mut llm_required = HashSet::new();
        llm_required.insert(Feature::ComplexReasoning);
        llm_required.insert(Feature::FreeFormEditing);
        llm_required.insert(Feature::CodeGeneration);
        llm_required.insert(Feature::ContextualSuggestions);

        Self {
            llm_required,
            disabled: HashSet::new(),
            bypassed: HashMap::new(),
            audit_log: Vec::new(),
            bypass_duration: 300, // 5 minutes
        }
    }

    /// Check if a feature is available in the given mode
    pub fn is_available(&self, feature: &Feature, mode: EditorMode) -> bool {
        // Explicitly disabled features are never available
        if self.disabled.contains(feature) {
            return false;
        }

        // If feature doesn't require LLM, always available
        if !self.llm_required.contains(feature) {
            return true;
        }

        // In LLM mode, all non-disabled features are available
        if mode == EditorMode::Llm {
            return true;
        }

        // In RuleBased mode, check for active bypass
        let now = crate::types::current_timestamp();
        if let Some(&expiry) = self.bypassed.get(feature) {
            if now < expiry {
                return true;
            }
        }

        false
    }

    /// Get the reason a feature is limited
    pub fn limit_reason(&self, feature: &Feature, mode: EditorMode) -> Option<LimitReason> {
        if self.disabled.contains(feature) {
            return Some(LimitReason::UserDisabled);
        }
        if self.llm_required.contains(feature) && mode != EditorMode::Llm {
            if let Some(&expiry) = self.bypassed.get(feature) {
                if crate::types::current_timestamp() >= expiry {
                    return Some(LimitReason::RequiresLlm);
                }
                return None; // Bypassed
            }
            return Some(LimitReason::RequiresLlm);
        }
        None
    }

    /// Temporarily bypass a feature restriction with user confirmation
    ///
    /// Returns true if bypass was granted, false if feature is not limitable.
    pub fn bypass(&mut self, feature: &Feature, user_confirmed: bool) -> bool {
        // Can only bypass LLM-required features
        if !self.llm_required.contains(feature) {
            return false;
        }

        let now = crate::types::current_timestamp();
        self.bypassed
            .insert(feature.clone(), now + self.bypass_duration);

        self.audit_log.push(BypassEntry {
            feature: feature.clone(),
            timestamp: now,
            user_confirmed,
        });

        // Keep audit log bounded
        if self.audit_log.len() > 200 {
            self.audit_log.remove(0);
        }

        true
    }

    /// Get all features with their availability status
    pub fn feature_status(&self, mode: EditorMode) -> Vec<(Feature, bool)> {
        let all_features = [
            Feature::ComplexReasoning,
            Feature::FreeFormEditing,
            Feature::CodeGeneration,
            Feature::ContextualSuggestions,
            Feature::VisualUnderstanding,
            Feature::ProceduralLearning,
        ];
        all_features
            .iter()
            .map(|f| (f.clone(), self.is_available(f, mode)))
            .collect()
    }

    /// Get features that are currently limited
    pub fn limited_features(&self, mode: EditorMode) -> Vec<(Feature, LimitReason)> {
        let all_features = [
            Feature::ComplexReasoning,
            Feature::FreeFormEditing,
            Feature::CodeGeneration,
            Feature::ContextualSuggestions,
            Feature::VisualUnderstanding,
            Feature::ProceduralLearning,
        ];
        all_features
            .iter()
            .filter_map(|f| self.limit_reason(f, mode).map(|r| (f.clone(), r)))
            .collect()
    }

    /// Get the audit log
    pub fn audit_log(&self) -> &[BypassEntry] {
        &self.audit_log
    }

    /// Clean up expired bypasses
    pub fn cleanup_expired(&mut self) {
        let now = crate::types::current_timestamp();
        self.bypassed.retain(|_, &mut expiry| now < expiry);
    }
}

impl Default for FeatureLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_features_available_in_llm_mode() {
        let limiter = FeatureLimiter::new();
        assert!(limiter.is_available(&Feature::ComplexReasoning, EditorMode::Llm));
        assert!(limiter.is_available(&Feature::FreeFormEditing, EditorMode::Llm));
    }

    #[test]
    fn test_llm_features_limited_in_rule_mode() {
        let limiter = FeatureLimiter::new();
        assert!(!limiter.is_available(&Feature::ComplexReasoning, EditorMode::RuleBased));
        assert!(!limiter.is_available(&Feature::FreeFormEditing, EditorMode::RuleBased));
    }

    #[test]
    fn test_non_llm_features_always_available() {
        let limiter = FeatureLimiter::new();
        assert!(limiter.is_available(&Feature::VisualUnderstanding, EditorMode::RuleBased));
        assert!(limiter.is_available(&Feature::ProceduralLearning, EditorMode::RuleBased));
    }

    #[test]
    fn test_bypass_grants_temporary_access() {
        let mut limiter = FeatureLimiter::new();
        assert!(!limiter.is_available(&Feature::ComplexReasoning, EditorMode::RuleBased));

        let granted = limiter.bypass(&Feature::ComplexReasoning, true);
        assert!(granted);
        assert!(limiter.is_available(&Feature::ComplexReasoning, EditorMode::RuleBased));
    }

    #[test]
    fn test_bypass_records_audit_log() {
        let mut limiter = FeatureLimiter::new();
        limiter.bypass(&Feature::ComplexReasoning, true);
        limiter.bypass(&Feature::FreeFormEditing, false);

        let log = limiter.audit_log();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].feature, Feature::ComplexReasoning);
        assert!(log[0].user_confirmed);
        assert_eq!(log[1].feature, Feature::FreeFormEditing);
        assert!(!log[1].user_confirmed);
    }

    #[test]
    fn test_cannot_bypass_non_llm_feature() {
        let mut limiter = FeatureLimiter::new();
        let granted = limiter.bypass(&Feature::VisualUnderstanding, true);
        assert!(!granted);
    }

    #[test]
    fn test_limit_reason() {
        let limiter = FeatureLimiter::new();
        assert_eq!(
            limiter.limit_reason(&Feature::ComplexReasoning, EditorMode::RuleBased),
            Some(LimitReason::RequiresLlm)
        );
        assert_eq!(
            limiter.limit_reason(&Feature::ComplexReasoning, EditorMode::Llm),
            None
        );
    }

    #[test]
    fn test_limited_features_list() {
        let limiter = FeatureLimiter::new();
        let limited = limiter.limited_features(EditorMode::RuleBased);
        assert_eq!(limited.len(), 4); // 4 LLM-required features
    }

    #[test]
    fn test_feature_status() {
        let limiter = FeatureLimiter::new();
        let status = limiter.feature_status(EditorMode::Llm);
        assert_eq!(status.len(), 6);
        assert!(status.iter().all(|(_, available)| *available));
    }
}

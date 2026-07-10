use super::RevisionType;

// ===========================================================================
// Observation Pattern - 观察结果模式
// ===========================================================================

/// A pattern that can be matched against tool execution observations.
#[derive(Debug, Clone)]
pub struct ObservationPattern {
    /// Pattern name for logging and debugging.
    pub name: String,
    /// Keywords that trigger this pattern (case-insensitive).
    pub keywords: Vec<String>,
    /// Regex pattern for more complex matching (optional).
    pub regex_pattern: Option<String>,
    /// The type of revision to apply when matched.
    pub revision_type_fn: fn(&str, usize) -> Option<RevisionType>,
    /// Confidence score (0.0-1.0) for this pattern match.
    pub confidence: f32,
}

impl ObservationPattern {
    /// Check if this observation matches the pattern.
    pub fn matches(&self, observation: &str) -> Option<(RevisionType, f32)> {
        let obs_lower = observation.to_lowercase();

        // Keyword matching
        let keyword_match = self
            .keywords
            .iter()
            .any(|kw| obs_lower.contains(&kw.to_lowercase()));

        if keyword_match {
            // Try to generate revision using the function
            if let Some(revision) = (self.revision_type_fn)(observation, 0) {
                return Some((revision, self.confidence));
            }
        }

        None
    }
}

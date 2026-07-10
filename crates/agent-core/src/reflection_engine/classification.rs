//! Error Classification - 错误分类

/// Classification of execution errors for appropriate response.
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorClassification {
    /// Transient error that may succeed on retry (network timeout, rate limit).
    Transient {
        /// Suggested retry delay in milliseconds.
        retry_delay_ms: u64,
        /// Maximum number of retries recommended.
        max_retries: usize,
    },
    /// Fatal error that will not succeed without intervention (invalid parameters).
    Fatal { reason: String },
    /// Permission error requiring user approval or privilege escalation.
    Permission {
        required_privilege: String,
        can_degrade: bool,
    },
    /// Invalid input error suggesting parameter adjustment.
    InvalidInput {
        parameter_name: String,
        suggestion: String,
    },
    /// Entity state error (not found, already exists, wrong type).
    EntityState {
        entity_name: String,
        expected_state: String,
        actual_state: String,
    },
}

impl ErrorClassification {
    /// Check if this error is recoverable through automatic means.
    pub fn is_auto_recoverable(&self) -> bool {
        matches!(
            self,
            ErrorClassification::Transient { .. }
                | ErrorClassification::InvalidInput { .. }
                | ErrorClassification::EntityState { .. }
                | ErrorClassification::Permission {
                    can_degrade: true,
                    ..
                }
        )
    }

    /// Check if this error requires user intervention.
    pub fn requires_user_intervention(&self) -> bool {
        matches!(
            self,
            ErrorClassification::Permission {
                can_degrade: false,
                ..
            }
        )
    }

    /// Get human-readable description of this classification.
    pub fn describe(&self) -> String {
        match self {
            ErrorClassification::Transient {
                retry_delay_ms,
                max_retries,
            } => {
                format!(
                    "Transient error (retry up to {}x with {}ms delay)",
                    max_retries, retry_delay_ms
                )
            }
            ErrorClassification::Fatal { reason } => {
                format!("Fatal error: {}", reason)
            }
            ErrorClassification::Permission {
                required_privilege,
                can_degrade,
            } => {
                if *can_degrade {
                    format!(
                        "Permission error (need '{}', can degrade to lower-risk approach)",
                        required_privilege
                    )
                } else {
                    format!(
                        "Permission error (requires '{}' from user)",
                        required_privilege
                    )
                }
            }
            ErrorClassification::InvalidInput {
                parameter_name,
                suggestion,
            } => {
                format!("Invalid input for '{}': {}", parameter_name, suggestion)
            }
            ErrorClassification::EntityState {
                entity_name,
                expected_state,
                actual_state,
            } => {
                format!(
                    "Entity '{}' state mismatch: expected {}, got {}",
                    entity_name, expected_state, actual_state
                )
            }
        }
    }
}

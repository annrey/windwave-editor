//! Reflection Engine - 核心反思引擎

use super::classification::ErrorClassification;
use super::helpers;
use super::reflection_entry::ReflectionEntry;
use super::retry_config::RetryConfig;
use super::stats::ReflectionStats;

/// Intelligent reflection engine for automatic error recovery and retry.
///
/// ## Usage
///
/// ```rust
/// # use agent_core::ReflectionEngine;
/// let mut engine = ReflectionEngine::new();
///
/// // Classify an error to determine recovery strategy:
/// let classification = engine.classify_error("Connection timed out");
/// assert!(classification.is_auto_recoverable());
///
/// // Generate a reflection analysis:
/// let reflection = engine.generate_reflection(
///     "create_entity",
///     "Connection timed out after 30s",
///     &classification,
/// );
/// assert!(reflection.contains("retry"));
/// ```
pub struct ReflectionEngine {
    /// Configuration for retry behavior.
    retry_config: RetryConfig,
    /// History of all reflections.
    reflection_history: Vec<ReflectionEntry>,
    /// Total number of successful reflections (for metrics).
    pub(crate) successful_reflections: usize,
    /// Total number of failed reflections (for metrics).
    pub(crate) failed_reflections: usize,
}

impl ReflectionEngine {
    /// Create a new ReflectionEngine with default configuration.
    pub fn new() -> Self {
        Self::with_config(RetryConfig::default())
    }

    /// Create a new ReflectionEngine with custom configuration.
    pub fn with_config(config: RetryConfig) -> Self {
        Self {
            retry_config: config,
            reflection_history: Vec::new(),
            successful_reflections: 0,
            failed_reflections: 0,
        }
    }

    /// Classify an error message into an appropriate category.
    ///
    /// Uses keyword matching and pattern recognition to determine
    /// the best course of action for handling the error.
    pub fn classify_error(&self, error: &str) -> ErrorClassification {
        let error_lower = error.to_lowercase();

        // === Transient Errors (Network/Timeout) ===
        if error_lower.contains("timeout")
            || error_lower.contains("超时")
            || error_lower.contains("timed out")
            || error_lower.contains("连接超时")
        {
            return ErrorClassification::Transient {
                retry_delay_ms: 500,
                max_retries: self.retry_config.max_retries,
            };
        }

        if error_lower.contains("connection refused")
            || error_lower.contains("连接被拒")
            || error_lower.contains("network error")
            || error_lower.contains("网络错误")
            || error_lower.contains("econnrefused")
            || error_lower.contains("etimedout")
        {
            return ErrorClassification::Transient {
                retry_delay_ms: 1000,
                max_retries: self.retry_config.max_retries,
            };
        }

        if error_lower.contains("rate limit")
            || error_lower.contains("速率限制")
            || error_lower.contains("too many requests")
            || error_lower.contains("请求过多")
            || error_lower.contains("429")
            || error_lower.contains("quota exceeded")
        {
            return ErrorClassification::Transient {
                retry_delay_ms: 2000,
                max_retries: self.retry_config.max_retries,
            };
        }

        // === Permission Errors ===
        if error_lower.contains("permission denied")
            || error_lower.contains("权限不足")
            || error_lower.contains("access denied")
            || error_lower.contains("拒绝访问")
            || error_lower.contains("unauthorized")
            || error_lower.contains("未授权")
            || error_lower.contains("403")
            || error_lower.contains("forbidden")
        {
            return ErrorClassification::Permission {
                required_privilege: "admin".into(),
                can_degrade: true,
            };
        }

        // === Entity State Errors ===
        if error_lower.contains("not found")
            || error_lower.contains("未找到")
            || error_lower.contains("does not exist")
            || error_lower.contains("不存在")
            || error_lower.contains("no such entity")
            || error_lower.contains("404")
        {
            let entity_name = helpers::extract_entity_from_error(error);
            return ErrorClassification::EntityState {
                entity_name,
                expected_state: "exists".into(),
                actual_state: "not found".into(),
            };
        }

        if error_lower.contains("already exists")
            || error_lower.contains("已存在")
            || error_lower.contains("duplicate")
            || error_lower.contains("重复")
            || error_lower.contains("409")
            || error_lower.contains("conflict")
        {
            let entity_name = helpers::extract_entity_from_error(error);
            return ErrorClassification::EntityState {
                entity_name,
                expected_state: "not exists".into(),
                actual_state: "already exists".into(),
            };
        }

        // === Invalid Input Errors ===
        if error_lower.contains("invalid")
            || error_lower.contains("无效")
            || error_lower.contains("parameter")
            || error_lower.contains("参数")
            || error_lower.contains("bad request")
            || error_lower.contains("400")
        {
            return ErrorClassification::InvalidInput {
                parameter_name: helpers::extract_parameter_name(error)
                    .unwrap_or_else(|| "unknown".into()),
                suggestion: "Check parameter values and types".into(),
            };
        }

        // === Default: Fatal Error ===
        ErrorClassification::Fatal {
            reason: helpers::truncate_str(error, 200),
        }
    }

    /// Generate a reflection text explaining what went wrong and how to fix it.
    ///
    /// This simulates the agent's reasoning process about the failure.
    pub fn generate_reflection(
        &self,
        original_action: &str,
        error: &str,
        classification: &ErrorClassification,
    ) -> String {
        let mut parts = Vec::new();

        parts.push("**What went wrong?**".to_string());
        parts.push(format!(
            "Action '{}' failed with error: {}",
            original_action,
            helpers::truncate_str(error, 150)
        ));

        parts.push(String::new());
        parts.push("**Why did it fail?**".to_string());
        match classification {
            ErrorClassification::Transient { .. } => {
                parts.push(
                    "This appears to be a temporary issue (network timeout, rate limiting).".into(),
                );
                parts.push("These errors often resolve themselves after a short wait.".into());
            }
            ErrorClassification::Fatal { reason } => {
                parts.push(format!(
                    "This is a fatal error that cannot be automatically recovered: {}",
                    reason
                ));
                parts.push("The operation cannot proceed without fundamental changes.".into());
            }
            ErrorClassification::Permission {
                required_privilege,
                can_degrade,
            } => {
                parts.push(format!(
                    "Insufficient privileges: need '{}'",
                    required_privilege
                ));
                if *can_degrade {
                    parts.push("However, we may be able to use a lower-risk approach.".into());
                } else {
                    parts.push("User approval is required to proceed.".into());
                }
            }
            ErrorClassification::InvalidInput {
                parameter_name,
                suggestion,
            } => {
                parts.push(format!("Parameter '{}' has invalid value", parameter_name));
                parts.push(format!("Suggestion: {}", suggestion));
            }
            ErrorClassification::EntityState {
                entity_name,
                expected_state,
                actual_state,
            } => {
                parts.push(format!("Entity '{}' is in unexpected state", entity_name));
                parts.push(format!(
                    "Expected: {}, Actual: {}",
                    expected_state, actual_state
                ));
            }
        }

        parts.push(String::new());
        parts.push("**How to fix it?**".to_string());

        match classification {
            ErrorClassification::Transient { .. } => {
                parts.push("→ Wait briefly and retry the operation".into());
                parts.push("→ Use exponential backoff to avoid overwhelming the service".into());
            }
            ErrorClassification::Fatal { .. } => {
                parts.push("→ Abort this operation and report to user".into());
                parts.push("→ Suggest alternative approach if possible".into());
            }
            ErrorClassification::Permission { can_degrade, .. } => {
                if *can_degrade {
                    parts.push(
                        "→ Try a lower-risk approach that doesn't require elevated privileges"
                            .into(),
                    );
                    parts.push("→ Request user confirmation for the degraded operation".into());
                } else {
                    parts.push("→ Request explicit user approval".into());
                    parts.push("→ Provide clear explanation of why permission is needed".into());
                }
            }
            ErrorClassification::InvalidInput { suggestion, .. } => {
                parts.push(format!("→ {}", suggestion));
                parts.push("→ Validate inputs before retrying".into());
            }
            ErrorClassification::EntityState {
                entity_name,
                actual_state,
                ..
            } => {
                if actual_state == "not found" {
                    parts.push(format!(
                        "→ Create entity '{}' first before operating on it",
                        entity_name
                    ));
                } else if actual_state == "already exists" {
                    parts.push(format!(
                        "→ Skip creation and modify existing entity '{}'",
                        entity_name
                    ));
                } else {
                    parts.push(format!(
                        "→ Adjust operation to account for current state of '{}'",
                        entity_name
                    ));
                }
            }
        }

        parts.join("\n")
    }

    /// Generate an alternative strategy based on the error classification.
    ///
    /// Returns None if no alternative is available.
    pub fn generate_alternative_strategy(
        &self,
        original_action: &str,
        error: &str,
        classification: &ErrorClassification,
    ) -> Option<String> {
        let original_lower = original_action.to_lowercase();
        let _error_lower = error.to_lowercase();

        match classification {
            ErrorClassification::Transient { .. } => Some(format!(
                "Retry '{}' with exponential backoff",
                original_action
            )),

            ErrorClassification::Permission { can_degrade, .. } => {
                if *can_degrade {
                    Some(format!(
                        "[LOW_RISK] {} (degraded due to permission)",
                        original_action
                    ))
                } else {
                    None
                }
            }

            ErrorClassification::EntityState {
                entity_name,
                actual_state,
                ..
            } => {
                if actual_state == "not found" {
                    if original_lower.contains("delete") || original_lower.contains("删除") {
                        Some(format!(
                            "Skip deletion of non-existent entity '{}'",
                            entity_name
                        ))
                    } else if original_lower.contains("update")
                        || original_lower.contains("修改")
                        || original_lower.contains("move")
                        || original_lower.contains("移动")
                    {
                        Some(format!(
                            "Create entity '{}' first, then apply modification",
                            entity_name
                        ))
                    } else {
                        Some(format!("Create missing entity '{}'", entity_name))
                    }
                } else if actual_state == "already exists" {
                    if original_lower.contains("create")
                        || original_lower.contains("创建")
                        || original_lower.contains("生成")
                    {
                        let modified = if original_action.contains("Create")
                            || original_action.contains("create")
                        {
                            original_action
                                .replace("Create", "Modify")
                                .replace("create", "modify")
                        } else if original_action.contains("创建") {
                            original_action.replace("创建", "修改")
                        } else if original_action.contains("生成") {
                            original_action.replace("生成", "更新")
                        } else {
                            format!("modify_entity('{}')", entity_name)
                        };
                        Some(modified)
                    } else {
                        Some(format!(
                            "Use existing entity '{}' instead of creating new one",
                            entity_name
                        ))
                    }
                } else {
                    Some(format!(
                        "Adjust operation for entity '{}' in its current state",
                        entity_name
                    ))
                }
            }

            ErrorClassification::InvalidInput {
                parameter_name,
                suggestion,
                ..
            } => Some(format!(
                "Fix parameter '{}': {}",
                parameter_name, suggestion
            )),

            ErrorClassification::Fatal { .. } => None,
        }
    }

    /// Execute an operation with automatic retry and reflection.
    ///
    /// This is the main entry point for using the ReflectionEngine.
    ///
    /// # Type Parameters
    /// * `F` - Future-like type representing the async operation
    /// * `T` - Return type of the operation
    ///
    /// # Arguments
    /// * `operation` - Async function to execute (will be called multiple times on retry)
    /// * `action_name` - Human-readable name of the action (for logging)
    ///
    /// # Returns
    /// * `Ok(T)` - Operation succeeded (possibly after retries)
    /// * `Err(String)` - Operation failed after all retries or unrecoverable error
    #[allow(dead_code)] // Will be used once integrated
    pub async fn execute_with_retry<F, T, Fut>(
        &mut self,
        operation: F,
        action_name: &str,
    ) -> Result<T, String>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, String>>,
    {
        let mut last_error = String::new();
        let mut reflection_entry: Option<ReflectionEntry> = None;

        for attempt in 0..=self.retry_config.max_retries {
            // Attempt the operation
            match operation().await {
                Ok(result) => {
                    // Success!
                    if let Some(ref mut entry) = reflection_entry {
                        entry.mark_resolved(
                            &format!("Retry #{} succeeded", attempt),
                            attempt,
                            0, // Would need real timing here
                        );
                        self.reflection_history.push(entry.clone());
                        self.successful_reflections += 1;
                    }

                    eprintln!(
                        "[Reflection] Action '{}' succeeded on attempt {}/{}",
                        action_name,
                        attempt + 1,
                        self.retry_config.max_retries + 1
                    );

                    return Ok(result);
                }

                Err(error) => {
                    last_error = error.clone();

                    // Classify the error
                    let classification = self.classify_error(&error);

                    eprintln!(
                        "[Reflection] Action '{}' failed on attempt {}/{}: [{}] {}",
                        action_name,
                        attempt + 1,
                        self.retry_config.max_retries + 1,
                        classification.describe(),
                        helpers::truncate_str(&error, 100)
                    );

                    // Generate reflection (only on first failure)
                    if reflection_entry.is_none() {
                        let reflection_text =
                            self.generate_reflection(action_name, &error, &classification);

                        reflection_entry = Some(ReflectionEntry::new(
                            action_name,
                            &error,
                            classification.clone(),
                            &reflection_text,
                        ));

                        eprintln!("[Reflection] Analysis:\n{}", reflection_text);
                    }

                    // Check if we should retry
                    if !classification.is_auto_recoverable() {
                        eprintln!(
                            "[Reflection] Non-recoverable error '{}', aborting",
                            classification.describe()
                        );
                        break;
                    }

                    // Check if we've exhausted retries
                    if attempt >= self.retry_config.max_retries {
                        eprintln!(
                            "[Reflection] Max retries ({}) reached",
                            self.retry_config.max_retries
                        );
                        break;
                    }

                    // Wait before retrying (exponential backoff)
                    let delay = self.retry_config.calculate_delay(attempt);
                    eprintln!(
                        "[Reflection] Waiting {:?} before retry #{}",
                        delay,
                        attempt + 1
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }

        // All retries exhausted or non-recoverable error
        if let Some(mut entry) = reflection_entry {
            entry.resolved = false;
            entry.retry_count = self.retry_config.max_retries + 1;
            self.reflection_history.push(entry.clone());
            self.failed_reflections += 1;
        }

        Err(format!(
            "Action '{}' failed after {} attempts: {}",
            action_name,
            self.retry_config.max_retries + 1,
            last_error
        ))
    }

    /// Get all reflection history entries.
    pub fn get_reflection_history(&self) -> &[ReflectionEntry] {
        &self.reflection_history
    }

    /// Get statistics about reflection performance.
    pub fn get_stats(&self) -> ReflectionStats {
        ReflectionStats {
            total_reflections: self.successful_reflections + self.failed_reflections,
            successful: self.successful_reflections,
            failed: self.failed_reflections,
            success_rate: if self.successful_reflections + self.failed_reflections > 0 {
                self.successful_reflections as f64
                    / (self.successful_reflections + self.failed_reflections) as f64
            } else {
                0.0
            },
        }
    }

    /// Clear reflection history (for testing or new session).
    pub fn clear_history(&mut self) {
        self.reflection_history.clear();
        self.successful_reflections = 0;
        self.failed_reflections = 0;
    }
}

impl Default for ReflectionEngine {
    fn default() -> Self {
        Self::new()
    }
}

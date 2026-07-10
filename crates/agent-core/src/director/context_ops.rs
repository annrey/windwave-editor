//! Context building, edit history, and audit operations for DirectorRuntime

use crate::director::DirectorRuntime;

impl DirectorRuntime {
    /// Build a complete L0-L3 layered context from current runtime state.
    pub fn build_layered_context(
        &self,
        user_request: Option<&str>,
    ) -> crate::prompt::LayeredContext {
        let mut builder = crate::layered_context_builder::LayeredContextBuilder::new()
            .with_engine("Bevy")
            .with_project("AgentEdit");

        if let Some(req) = user_request {
            builder = builder.with_user_request(req);
        }

        builder = builder.with_recent_actions(
            self.trace_entries
                .iter()
                .map(|t| t.summary.clone())
                .collect(),
        );

        if let Some(ref bridge) = self.scene_bridge {
            builder = builder.with_scene_bridge(bridge.as_ref());
        }

        builder.build()
    }

    /// Build a context prompt string from the current layered context.
    pub fn build_context_prompt(&self, user_request: Option<&str>) -> String {
        let layered = self.build_layered_context(user_request);
        let builder = crate::layered_context_builder::LayeredContextBuilder::new();
        builder.build_prompt(&layered)
    }

    /// Access the EditHistory for fine-grained undo/redo.
    pub fn edit_history(&self) -> &crate::edit_history::EditHistory {
        &self.edit_history
    }

    /// Access the EditHistory mutably for fine-grained undo/redo operations.
    pub fn edit_history_mut(&mut self) -> &mut crate::edit_history::EditHistory {
        &mut self.edit_history
    }

    /// Push an executed EditOp onto the undo history.
    pub fn push_edit_op(&mut self, op: Box<dyn crate::edit_ops::EditOp>) {
        self.edit_history.push(op);
    }

    /// Undo the most recent edit operation via the EditHistory.
    pub fn undo_edit(&mut self) -> Result<bool, crate::edit_ops::EditOpError> {
        if let Some(ref mut bridge) = self.scene_bridge {
            self.edit_history.undo(bridge.as_mut())
        } else {
            Err(crate::edit_ops::EditOpError::Bridge(
                "No SceneBridge connected".into(),
            ))
        }
    }

    /// Redo the most recently undone operation via the EditHistory.
    pub fn redo_edit(&mut self) -> Result<bool, crate::edit_ops::EditOpError> {
        if let Some(ref mut bridge) = self.scene_bridge {
            self.edit_history.redo(bridge.as_mut())
        } else {
            Err(crate::edit_ops::EditOpError::Bridge(
                "No SceneBridge connected".into(),
            ))
        }
    }

    /// Record an operation to the tamper-evident audit log.
    pub fn record_audit(
        &mut self,
        agent_id: u64,
        action: &str,
        target: &str,
        result: &str,
        risk_level: &str,
        user_approved: bool,
    ) {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.audit_log.record(
            ts,
            agent_id,
            action,
            target,
            result,
            risk_level,
            user_approved,
        );
    }

    /// Verify the integrity of the audit log chain.
    pub fn verify_audit_log(&self) -> bool {
        self.audit_log.verify()
    }

    /// Access the audit log for read-only inspection.
    pub fn audit_log(&self) -> &crate::audit::AuditLog {
        &self.audit_log
    }
}

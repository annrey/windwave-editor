use crate::director::types::EditorEvent;
use crate::memory::{
    MemorySystem, MemoryTier,
    EpisodeType,
};

/// Bridge that automatically extracts structured information from EditorEvent
/// streams and writes into the 4-layer memory system.
pub struct EventMemoryBridge {
    pub events_processed: usize,
    pub entities_discovered: usize,
    pub preferences_captured: usize,
    pub patterns_learned: usize,
    recent_entity_names: Vec<String>,
    preference_keywords: Vec<(&'static str, &'static str)>,
}

impl EventMemoryBridge {
    pub fn new() -> Self {
        Self {
            events_processed: 0,
            entities_discovered: 0,
            preferences_captured: 0,
            patterns_learned: 0,
            recent_entity_names: Vec::new(),
            preference_keywords: vec![
                ("颜色", "color"), ("红色", "red"), ("蓝色", "blue"),
                ("风格", "style"), ("大小", "size"), ("位置", "position"),
                ("字体", "font"), ("主题", "theme"),
            ],
        }
    }

    pub fn process_event(&mut self, event: &EditorEvent, memory: &mut MemorySystem) {
        self.events_processed += 1;

        match event {
            EditorEvent::EditPlanCreated { title, steps_count, risk, .. } => {
                let intent = format!("Executing plan: {} ({} steps, risk={})", title, steps_count, risk);
                memory.working.set_intent(&intent);
                let plan_id = memory.episodic.record_plan(title, *steps_count);
                eprintln!("[EventBridge] PlanCreated -> L3 intent + L2 episode(id={:?})", plan_id);
            }

            EditorEvent::StepStarted { title, .. } => {
                let hint = format!("Currently executing: {}", title);
                memory.working.add_hint(&hint);
            }

            EditorEvent::StepCompleted { title, result, .. } => {
                memory.episodic.record_step(title, result, true, 0);

                if let Some(entity_name) = Self::extract_entity_from_text(result) {
                    self.recent_entity_names.push(entity_name.clone());
                    if self.recent_entity_names.len() > 10 { self.recent_entity_names.remove(0); }
                    memory.working.register_entity(&entity_name, crate::types::EntityId(self.entities_discovered as u64));
                    self.entities_discovered += 1;
                    let desc = format!("Entity referenced in step '{}': {}", title, result);
                    let _ = memory.create_semantic_node(&entity_name, "entity", &desc);
                }
                let step_ctx = format!("Step: {}", title);
                let outcome = format!("Result: {}", result);
                memory.procedural.observe_decision(&step_ctx, "completed", &outcome, true);
                self.patterns_learned += 1;
            }

            EditorEvent::StepFailed { title, error, .. } => {
                let fail_msg = format!("FAILED: {}", error);
                memory.episodic.record_step(title, &fail_msg, false, 0);
                memory.episodic.record_error(error, Some(serde_json::json!({"context": format!("Step '{}' failed", title)})));
                let hint = format!("ERROR in '{}': {}", title, error);
                memory.working.add_hint(&hint);
                let step_ctx = format!("Step: {}", title);
                memory.procedural.observe_decision(&step_ctx, "failed", error, false);
                self.patterns_learned += 1;
            }

            EditorEvent::DirectExecutionStarted { request, .. } => {
                memory.working.set_intent(request);
                memory.episodic.record_plan(request, 1);
                if let Some(pref) = self.extract_preference(request) {
                    self.preferences_captured += 1;
                    memory.working.add_hint(&format!("Preference detected: {}={}", pref.0, pref.1));
                }
            }

            EditorEvent::DirectExecutionCompleted { request, success } => {
                if *success {
                    let msg = format!("{} [completed OK]", request);
                    memory.episodic.record_plan(&msg, 1);
                } else {
                    memory.episodic.record_error(&format!("Direct execution failed for: {}", request), None);
                }
            }

            EditorEvent::Error { message } => {
                let hint = format!("System error: {}", message);
                memory.working.add_hint(&hint);
                memory.episodic.record_error(message, None);
            }

            EditorEvent::ExecutionCompleted { success, .. } => {
                let outcome = if *success { "SUCCESS" } else { "FAILED" };
                let hint = format!("Plan execution finished: {}", outcome);
                memory.working.add_hint(&hint);
                if *success {
                    if memory.procedural.workflow_count() > 0 {
                        memory.procedural.record_use("plan_execution", true);
                    }
                }
            }

            EditorEvent::PermissionResolved { approved, reason, .. } => {
                let summary = if *approved {
                    format!("User approved: {}", reason.as_deref().unwrap_or("approved"))
                } else {
                    format!("User rejected: {}", reason.as_deref().unwrap_or("rejected"))
                };
                let episode = crate::memory::Episode {
                    metadata: crate::memory::MemoryMetadata::new(0, MemoryTier::Episodic),
                    episode_type: EpisodeType::UserApproved,
                    summary,
                    details: serde_json::json!({ "approved": approved }),
                    entity_ids: vec![],
                    success: Some(*approved),
                    duration_ms: None,
                };
                memory.record_episode(episode);
            }

            EditorEvent::GoalChecked { all_matched, summary, .. } => {
                let status = if *all_matched { "PASS" } else { "FAIL" };
                let hint = format!("Goal check: {} - {}", status, summary);
                memory.working.add_hint(&hint);
            }

            EditorEvent::ReviewCompleted { decision, summary, .. } => {
                let hint = format!("Review result: {} - {}", decision, summary);
                memory.working.add_hint(&hint);
            }

            _ => {}
        }
    }

    pub fn process_events(&mut self, events: &[EditorEvent], memory: &mut MemorySystem) {
        for event in events {
            self.process_event(event, memory);
        }
    }

    fn extract_entity_from_text(text: &str) -> Option<String> {
        let lower = text.to_lowercase();
        if !lower.contains("entity") && !lower.contains("实体") && !lower.contains("created")
            && !lower.contains("deleted") && !lower.contains("updated") {
            return None;
        }
        let patterns = [
            r"'([^']+)'",
            r#"[""]([^""]+)[""]"#,
            r#"(?:entity|实体)\s+(?:named?\s+|id\s*=?\s*|['\"])(\w[\w\s]*?)(?:\s*(?:with|at|in)|[,.\n]|$)"#,
        ];
        for pat in &patterns {
            if let Ok(re) = regex_lite_match(pat, text) {
                if let Some(caps) = re {
                    if !caps.is_empty() { return Some(caps[0].clone()); }
                }
            }
        }
        let name_patterns = ["Player", "Enemy", "Boss", "NPC", "Wall", "Floor", "Camera", "UI"];
        for &name in &name_patterns {
            if text.contains(name) { return Some(name.to_string()); }
        }
        None
    }

    fn extract_preference(&self, text: &str) -> Option<(&'static str, String)> {
        let lower = text.to_lowercase();
        for &(keyword, key) in &self.preference_keywords {
            if lower.contains(keyword) {
                let value = Self::extract_value_after_keyword(text, keyword);
                return Some((key, value.unwrap_or_else(|| "detected".to_string())));
            }
        }
        None
    }

    fn extract_value_after_keyword(text: &str, keyword: &str) -> Option<String> {
        if let Some(pos) = text.find(keyword) {
            let after = &text[pos + keyword.len()..].trim_start();
            let value_chars: String = after.chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == '#')
                .collect();
            if !value_chars.is_empty() && value_chars.len() < 50 {
                return Some(value_chars);
            }
        }
        None
    }

    pub fn stats(&self) -> String {
        format!(
            "Events processed: {}, Entities discovered: {}, Preferences captured: {}, Patterns learned: {}",
            self.events_processed, self.entities_discovered, self.preferences_captured, self.patterns_learned
        )
    }
}

fn regex_lite_match(pattern: &str, text: &str) -> Result<Option<Vec<String>>, ()> {
    let mut names = Vec::new();
    let mut i = 0;
    let pat_bytes = pattern.as_bytes();
    let txt_bytes = text.as_bytes();
    while i < pat_bytes.len() {
        match pat_bytes[i] {
            b'(' => {
                let mut capture = String::new();
                i += 1;
                while i < pat_bytes.len() && pat_bytes[i] != b')' {
                    match pat_bytes[i] {
                        b'\\' if i + 1 < pat_bytes.len() => {
                            i += 1;
                            capture.push(pat_bytes[i] as char);
                        }
                        b'.' => capture.push('.'),
                        b'[' => {
                            i += 1;
                            let mut char_class = Vec::new();
                            while i < pat_bytes.len() && pat_bytes[i] != b']' {
                                char_class.push(pat_bytes[i]);
                                i += 1;
                            }
                        }
                        c => capture.push(c as char),
                    }
                    i += 1;
                }
                names.push(capture);
            }
            b')' | b'\\' => { i += 1; }
            b'+' | b'*' | b'?' => { i += 1; }
            c => {
                if i < txt_bytes.len() && txt_bytes[i] == c { i += 1; } else { return Ok(None); }
            }
        }
    }
    if names.is_empty() { Ok(None) } else { Ok(Some(names)) }
}

impl Default for EventMemoryBridge {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod event_bridge_tests {
    use super::*;
    use crate::director::types::EditorEvent;
    use crate::memory::MemorySystem;

    #[test]
    fn test_bridge_new() {
        let bridge = EventMemoryBridge::new();
        assert_eq!(bridge.events_processed, 0);
        assert_eq!(bridge.entities_discovered, 0);
        assert_eq!(bridge.preferences_captured, 0);
        assert_eq!(bridge.patterns_learned, 0);
        assert!(bridge.recent_entity_names.is_empty());
        assert!(!bridge.preference_keywords.is_empty());
    }

    #[test]
    fn test_bridge_stats() {
        let bridge = EventMemoryBridge::new();
        let stats = bridge.stats();
        assert!(stats.contains("Events processed: 0"));
        assert!(stats.contains("Entities discovered: 0"));
    }

    #[test]
    fn test_bridge_default() {
        let bridge = EventMemoryBridge::default();
        assert_eq!(bridge.events_processed, 0);
    }

    #[test]
    fn test_process_edit_plan_created() {
        let mut bridge = EventMemoryBridge::new();
        let mut memory = MemorySystem::default();
        let event = EditorEvent::EditPlanCreated {
            plan_id: "plan-001".into(),
            title: "Test Plan".into(),
            risk: "low".into(),
            mode: "plan".into(),
            steps_count: 3,
        };
        bridge.process_event(&event, &mut memory);
        assert_eq!(bridge.events_processed, 1);
    }

    #[test]
    fn test_process_step_started() {
        let mut bridge = EventMemoryBridge::new();
        let mut memory = MemorySystem::default();
        let event = EditorEvent::StepStarted {
            plan_id: "plan-001".into(),
            step_id: "step-0".into(),
            title: "Create Player".into(),
        };
        bridge.process_event(&event, &mut memory);
        assert_eq!(bridge.events_processed, 1);
    }

    #[test]
    fn test_process_step_completed_with_entity() {
        let mut bridge = EventMemoryBridge::new();
        let mut memory = MemorySystem::default();
        let event = EditorEvent::StepCompleted {
            plan_id: "plan-001".into(),
            step_id: "step-0".into(),
            title: "Create Entity".into(),
            result: "Entity 'Player' created successfully".into(),
        };
        bridge.process_event(&event, &mut memory);
        assert_eq!(bridge.events_processed, 1);
        assert_eq!(bridge.entities_discovered, 1);
        assert!(bridge.patterns_learned > 0);
    }

    #[test]
    fn test_process_step_failed() {
        let mut bridge = EventMemoryBridge::new();
        let mut memory = MemorySystem::default();
        let event = EditorEvent::StepFailed {
            plan_id: "plan-001".into(),
            step_id: "step-0".into(),
            title: "Delete Entity".into(),
            error: "Entity not found".into(),
        };
        bridge.process_event(&event, &mut memory);
        assert_eq!(bridge.events_processed, 1);
        assert!(bridge.patterns_learned > 0);
    }

    #[test]
    fn test_process_error_event() {
        let mut bridge = EventMemoryBridge::new();
        let mut memory = MemorySystem::default();
        let event = EditorEvent::Error { message: "Connection timeout".into() };
        bridge.process_event(&event, &mut memory);
        assert_eq!(bridge.events_processed, 1);
    }

    #[test]
    fn test_process_execution_completed_success() {
        let mut bridge = EventMemoryBridge::new();
        let mut memory = MemorySystem::default();
        let event = EditorEvent::ExecutionCompleted { plan_id: "plan-001".into(), success: true };
        bridge.process_event(&event, &mut memory);
        assert_eq!(bridge.events_processed, 1);
    }

    #[test]
    fn test_process_execution_completed_failure() {
        let mut bridge = EventMemoryBridge::new();
        let mut memory = MemorySystem::default();
        let event = EditorEvent::ExecutionCompleted { plan_id: "plan-001".into(), success: false };
        bridge.process_event(&event, &mut memory);
        assert_eq!(bridge.events_processed, 1);
    }

    #[test]
    fn test_process_permission_resolved() {
        let mut bridge = EventMemoryBridge::new();
        let mut memory = MemorySystem::default();
        let event = EditorEvent::PermissionResolved {
            plan_id: "plan-001".into(),
            approved: true,
            reason: Some("Looks good".into()),
        };
        bridge.process_event(&event, &mut memory);
        assert_eq!(bridge.events_processed, 1);
    }

    #[test]
    fn test_process_batch() {
        let mut bridge = EventMemoryBridge::new();
        let mut memory = MemorySystem::default();
        let events = vec![
            EditorEvent::StepStarted { plan_id: "p1".into(), step_id: "s1".into(), title: "Step 1".into() },
            EditorEvent::StepCompleted { plan_id: "p1".into(), step_id: "s1".into(), title: "Step 1".into(), result: "Done".into() },
            EditorEvent::ExecutionCompleted { plan_id: "p1".into(), success: true },
        ];
        bridge.process_events(&events, &mut memory);
        assert_eq!(bridge.events_processed, 3);
    }

    #[test]
    fn test_extract_entity_from_text() {
        assert_eq!(
            EventMemoryBridge::extract_entity_from_text("Entity 'Player' created"),
            Some("Player".to_string())
        );
        assert_eq!(
            EventMemoryBridge::extract_entity_from_text("created Enemy at position"),
            Some("Enemy".to_string())
        );
        assert_eq!(
            EventMemoryBridge::extract_entity_from_text("nothing special here"),
            None
        );
    }

    #[test]
    fn test_extract_preference_color() {
        let bridge = EventMemoryBridge::new();
        let pref = bridge.extract_preference("我想要一个红色的玩家");
        assert!(pref.is_some());
        let (key, _value) = pref.unwrap();
        assert_eq!(key, "red");
    }

    #[test]
    fn test_extract_preference_none() {
        let bridge = EventMemoryBridge::new();
        let pref = bridge.extract_preference("hello world");
        assert!(pref.is_none());
    }

    #[test]
    fn test_direct_execution_with_preference() {
        let mut bridge = EventMemoryBridge::new();
        let mut memory = MemorySystem::default();
        let event = EditorEvent::DirectExecutionStarted {
            request: "创建一个红色敌人".into(),
            mode: "direct".into(),
            complexity_score: 3,
        };
        bridge.process_event(&event, &mut memory);
        assert_eq!(bridge.events_processed, 1);
        assert_eq!(bridge.preferences_captured, 1);
    }
}

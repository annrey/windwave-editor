use crate::plan::EditPlan;

#[derive(Debug, Clone)]
pub struct AgentSnapshot {
    pub request: String,
    pub plan: Option<EditPlan>,
    pub available_tools: Vec<String>,
    pub scene_entities: Vec<String>,
    pub timestamp: u64,
}

impl AgentSnapshot {
    pub fn new(
        request: String,
        plan: Option<EditPlan>,
        available_tools: Vec<String>,
        scene_entities: Vec<String>,
    ) -> Self {
        Self {
            request,
            plan,
            available_tools,
            scene_entities,
            timestamp: crate::types::current_timestamp(),
        }
    }

    pub fn snapshot_id(&self) -> String {
        format!(
            "snapshot_{}_{}",
            self.timestamp,
            self.request
                .chars()
                .take(30)
                .collect::<String>()
                .replace(' ', "_")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_creation() {
        let snapshot = AgentSnapshot::new(
            "test request".to_string(),
            None,
            vec!["create_entity".to_string()],
            vec!["Player".to_string()],
        );
        assert_eq!(snapshot.request, "test request");
        assert!(snapshot.plan.is_none());
        assert!(snapshot.timestamp > 0);
        assert_eq!(snapshot.available_tools.len(), 1);
    }

    #[test]
    fn test_snapshot_id_format() {
        let snapshot = AgentSnapshot::new("create red enemy".to_string(), None, vec![], vec![]);
        let id = snapshot.snapshot_id();
        assert!(id.starts_with("snapshot_"));
        assert!(id.contains("create_red_enemy"));
    }
}

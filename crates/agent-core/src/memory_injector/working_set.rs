use serde::{Deserialize, Serialize};
use crate::types::current_timestamp;

/// 当前工作集 - 追踪用户当前在做什么、关注什么
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingSet {
    pub open_files: Vec<String>,
    pub recent_entities: Vec<String>,
    pub active_context: Vec<String>,
    pub current_task: Option<String>,
    pub last_updated: u64,
}

impl WorkingSet {
    pub fn new() -> Self {
        Self {
            open_files: Vec::new(),
            recent_entities: Vec::new(),
            active_context: Vec::new(),
            current_task: None,
            last_updated: current_timestamp(),
        }
    }

    pub fn set_open_files(&mut self, files: Vec<String>) {
        self.open_files = files;
        self.last_updated = current_timestamp();
    }

    pub fn select_entity(&mut self, entity_name: &str) {
        self.recent_entities.retain(|e| e != entity_name);
        self.recent_entities.insert(0, entity_name.to_string());
        if self.recent_entities.len() > 5 {
            self.recent_entities.pop();
        }
        self.last_updated = current_timestamp();
    }

    pub fn set_task(&mut self, task: &str) {
        self.current_task = Some(task.to_string());
        self.last_updated = current_timestamp();
    }

    pub fn add_context(&mut self, context: &str) {
        if !self.active_context.contains(&context.to_string()) {
            self.active_context.push(context.to_string());
        }
        self.last_updated = current_timestamp();
    }

    pub fn describe(&self) -> String {
        let files_str = if self.open_files.is_empty() {
            "(none)".to_string()
        } else {
            self.open_files.join(", ")
        };
        let entities_str = if self.recent_entities.is_empty() {
            "(none)".to_string()
        } else {
            self.recent_entities.join(", ")
        };
        let context_str = if self.active_context.is_empty() {
            "(none)".to_string()
        } else {
            self.active_context.join(", ")
        };

        format!(
            "## 当前工作集\n\
             打开文件: {}\n\
             最近实体: {}\n\
             活跃上下文: {}\n\
             当前任务: {}",
            files_str,
            entities_str,
            context_str,
            self.current_task.as_deref().unwrap_or("(none)")
        )
    }
}

impl Default for WorkingSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_working_set() {
        let mut ws = WorkingSet::new();
        ws.select_entity("Player");
        ws.select_entity("Enemy");
        ws.set_task("调整玩家位置");
        let desc = ws.describe();
        assert!(desc.contains("Player"));
        assert!(desc.contains("Enemy"));
        assert!(desc.contains("调整玩家位置"));
    }
}

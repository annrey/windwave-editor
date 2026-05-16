//! ReAct tool execution — SceneBridge-based tool dispatch for the ReAct loop.

use super::types::DirectorRuntime;

impl DirectorRuntime {
    /// Sprint 1: Execute a ReAct tool call and return the observation string.
    pub(crate) async fn execute_react_tool(
        &mut self,
        tool_name: &str,
        parameters: &std::collections::HashMap<String, serde_json::Value>,
    ) -> String {
        if let Some(ref mut bridge) = self.scene_bridge {
            let result = match tool_name {
                "create_entity" | "spawn_entity" => {
                    let name = parameters
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("entity");
                    bridge.create_entity(name, None, &[])
                        .map(|id| format!("Created entity '{}' (id={})", name, id))
                        .map_err(|e| format!("Failed to create entity: {}", e))
                }
                "delete_entity" => {
                    let entity_id = parameters
                        .get("entity_id")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    bridge.delete_entity(entity_id)
                        .map(|()| format!("Deleted entity id={}", entity_id))
                        .map_err(|e| format!("Failed to delete entity: {}", e))
                }
                "update_component" | "set_transform" | "set_sprite" => {
                    let entity_id = parameters
                        .get("entity_id")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let comp_type = parameters
                        .get("component_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Transform");
                    let mut props = std::collections::HashMap::new();
                    if let Some(pos) = parameters.get("position") {
                        props.insert("position".into(), pos.clone());
                    }
                    if let Some(color) = parameters.get("color") {
                        props.insert("color".into(), color.clone());
                    }
                    bridge.update_component(entity_id, comp_type, props)
                        .map(|()| format!("Updated {} for entity id={}", comp_type, entity_id))
                        .map_err(|e| format!("Failed to update component: {}", e))
                }
                "query_entities" | "query_scene" => {
                    let entities = bridge.query_entities(None, None);
                    let names: Vec<String> = entities.iter().map(|e| e.name.clone()).collect();
                    Ok(format!("Scene entities ({}): {}", names.len(), names.join(", ")))
                }
                _ => {
                    Err(format!("Unknown tool: {}", tool_name))
                }
            };

            match result {
                Ok(msg) => msg,
                Err(e) => {
                    let error_msg = format!("Error: {}", e);
                    let classification = self.reflection_engine.classify_error(&e);

                    eprintln!(
                        "[ReflectionEngine] Tool '{}' failed: [{}] {}",
                        tool_name,
                        classification.describe(),
                        &e
                    );

                    let _reflection = self.reflection_engine.generate_reflection(
                        tool_name,
                        &e,
                        &classification,
                    );

                    if let Some(alt_strategy) = self.reflection_engine.generate_alternative_strategy(
                        tool_name,
                        &e,
                        &classification,
                    ) {
                        eprintln!(
                            "[ReflectionEngine] Alternative suggested: {}",
                            alt_strategy
                        );

                        self.trace_entries.push(super::types::DirectorTraceEntry {
                            timestamp_ms: crate::types::now_millis(),
                            actor: "ReflectionEngine".into(),
                            summary: format!(
                                "Failed '{}': {} → Suggested: {}",
                                tool_name, classification.describe(), alt_strategy
                            ),
                        });
                    }

                    error_msg
                }
            }
        } else {
            format!("Simulated: {} with params {:?} (no SceneBridge)", tool_name, parameters)
        }
    }
}

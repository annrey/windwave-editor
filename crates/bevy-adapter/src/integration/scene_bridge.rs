use super::*;

// ===========================================================================
// SceneIndexSceneBridge — implements agent_core::SceneBridge using SceneIndexCache
// ===========================================================================

/// Bridge that implements `SceneBridge` using a cached SceneIndex.
///
/// **Reads**: real data from `SceneIndexCache` (periodically rebuilt from ECS).
/// **Writes**: return success and accumulate `EngineCommand`s for later application.
pub struct SceneIndexSceneBridge {
    snapshot: Vec<CoreSceneEntityInfo>,
    entity_list: Vec<EntityListItem>,
    pending_writes: Vec<EngineCommand>,
    next_id: u64,
}

impl SceneIndexSceneBridge {
    /// Build a bridge from a SceneIndexCache snapshot.
    pub fn from_cache(cache: &SceneIndexCache) -> Self {
        let raw = cache.0.to_entity_info_list();
        let snapshot = raw
            .iter()
            .map(|info| CoreSceneEntityInfo {
                name: info.name.clone(),
                components: info.components.clone(),
                translation: info.translation,
                sprite_color: info.sprite_color,
            })
            .collect();

        let entity_list = raw
            .iter()
            .map(|info| EntityListItem {
                id: info.id,
                name: info.name.clone(),
                components: info.components.clone(),
            })
            .collect();

        let max_id = raw.iter().map(|i| i.id).max().unwrap_or(0);

        Self {
            snapshot,
            entity_list,
            pending_writes: Vec::new(),
            next_id: max_id + 1,
        }
    }

    /// Collect pending write commands to be applied by BevyAdapter.
    pub fn take_pending_writes(&mut self) -> Vec<EngineCommand> {
        std::mem::take(&mut self.pending_writes)
    }

    /// Number of pending write commands.
    pub fn pending_write_count(&self) -> usize {
        self.pending_writes.len()
    }
}

impl SceneBridge for SceneIndexSceneBridge {
    fn query_entities(
        &self,
        filter: Option<&str>,
        component_type: Option<&str>,
    ) -> Vec<EntityListItem> {
        self.entity_list
            .iter()
            .filter(|e| {
                let name_match = filter
                    .is_none_or(|f| f == "*" || e.name.to_lowercase().contains(&f.to_lowercase()));
                let comp_match =
                    component_type.is_none_or(|ct| e.components.iter().any(|c| c == ct));
                name_match && comp_match
            })
            .cloned()
            .collect()
    }

    fn get_entity(&self, id: u64) -> Option<serde_json::Value> {
        self.entity_list.iter().find(|e| e.id == id).map(|e| {
            let snap = self.snapshot.iter().find(|s| s.name == e.name);
            serde_json::json!({
                "id": e.id,
                "name": e.name,
                "components": e.components,
                "translation": snap.and_then(|s| s.translation),
                "sprite_color": snap.and_then(|s| s.sprite_color),
            })
        })
    }

    fn create_entity(
        &mut self,
        name: &str,
        position: Option<[f64; 2]>,
        components: &[ComponentPatch],
    ) -> Result<u64, String> {
        let id = self.next_id;
        self.next_id += 1;

        let mut bevy_components: Vec<crate::ComponentPatch> = Vec::new();
        for cp in components {
            bevy_components.push(crate::ComponentPatch {
                type_name: cp.type_name.clone(),
                value: serde_json::to_value(&cp.properties).unwrap_or_default(),
            });
        }

        self.pending_writes.push(EngineCommand::CreateEntity {
            name: name.to_string(),
            components: bevy_components,
        });

        if let Some(pos) = position {
            self.pending_writes.push(EngineCommand::SetTransform {
                entity_id: id,
                translation: Some([pos[0] as f32, pos[1] as f32, 0.0]),
                rotation: None,
                scale: None,
            });
        }

        let mut comp_names: Vec<String> = vec!["Transform".to_string()];
        let mut sprite_color: Option<[f32; 4]> = None;
        for cp in components {
            comp_names.push(cp.type_name.clone());
            if cp.type_name == "Sprite" {
                if let Some(color) = cp.properties.get("color").and_then(|v| v.as_array()) {
                    sprite_color = Some([
                        color.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                        color.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                        color.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                        color.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                    ]);
                }
            }
        }

        self.entity_list.push(EntityListItem {
            id,
            name: name.to_string(),
            components: comp_names.clone(),
        });

        self.snapshot.push(CoreSceneEntityInfo {
            name: name.to_string(),
            components: comp_names,
            translation: position.map(|p| [p[0] as f32, p[1] as f32, 0.0]),
            sprite_color,
        });

        Ok(id)
    }

    fn update_component(
        &mut self,
        entity_id: u64,
        component: &str,
        properties: std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        if component == "Transform" {
            if let Some(pos) = properties.get("position") {
                if let Some(arr) = pos.as_array() {
                    let x = arr.first().and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    let y = arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    self.pending_writes.push(EngineCommand::SetTransform {
                        entity_id,
                        translation: Some([x, y, 0.0]),
                        rotation: None,
                        scale: None,
                    });
                }
            }
        }
        if component == "Sprite" {
            if let Some(color) = properties.get("color") {
                if let Some(arr) = color.as_array() {
                    let r = arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                    let g = arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                    let b = arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                    let a = arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                    self.pending_writes.push(EngineCommand::SetSpriteColor {
                        entity_id,
                        rgba: [r, g, b, a],
                    });
                }
            }
        }
        if component == "Visibility" {
            if let Some(visible) = properties.get("visible") {
                let is_visible = visible.as_bool().unwrap_or(true);
                self.pending_writes.push(EngineCommand::SetVisibility {
                    entity_id,
                    visible: is_visible,
                });
            }
        }
        if component == "Name" {
            if let Some(name) = properties.get("name") {
                if let Some(new_name) = name.as_str() {
                    if let Some(entity) = self.entity_list.iter_mut().find(|e| e.id == entity_id) {
                        entity.name = new_name.to_string();
                    }
                }
            }
        }
        if component == "Parent" || component == "SetParent" {
            if let Some(parent_id) = properties.get("parent_id").and_then(|v| v.as_u64()) {
                self.pending_writes.push(EngineCommand::SetParent {
                    child_entity_id: entity_id,
                    parent_entity_id: parent_id,
                });
            }
        }
        Ok(())
    }

    fn delete_entity(&mut self, entity_id: u64) -> Result<(), String> {
        self.pending_writes
            .push(EngineCommand::DeleteEntity { entity_id });
        self.entity_list.retain(|e| e.id != entity_id);
        // Remove from snapshot to prevent ghost entries (CoreSceneEntityInfo has no id, match by name)
        let entity_name = self
            .entity_list
            .iter()
            .find(|e| e.id == entity_id)
            .map(|e| e.name.clone());
        if let Some(name) = entity_name {
            self.snapshot.retain(|e| e.name != name);
        }
        Ok(())
    }

    fn get_scene_snapshot(&self) -> Vec<CoreSceneEntityInfo> {
        self.snapshot.clone()
    }

    fn drain_commands(&mut self) -> Vec<serde_json::Value> {
        self.take_pending_writes()
            .into_iter()
            .map(|cmd| serde_json::to_value(cmd).unwrap_or(serde_json::Value::Null))
            .filter(|v| !v.is_null())
            .collect()
    }
}

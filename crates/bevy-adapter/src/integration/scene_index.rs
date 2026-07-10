use super::*;

// ===========================================================================
// SceneIndexRebuildPlugin — periodic scene snapshot for agent reasoning
// ===========================================================================

/// Cached SceneIndex, rebuilt periodically by `rebuild_scene_index` system.
#[derive(Resource, Clone, Default)]
pub struct SceneIndexCache(pub SceneIndex);

impl SceneIndexCache {
    /// Get a reference to the cached SceneIndex.
    pub fn get(&self) -> &SceneIndex {
        &self.0
    }

    /// Convert cached index to goal-checker compatible entity list.
    pub fn to_goal_checker_snapshot(&self) -> Vec<CoreSceneEntityInfo> {
        let raw = self.0.to_entity_info_list();
        raw.into_iter()
            .map(|info| CoreSceneEntityInfo {
                name: info.name,
                components: info.components,
                translation: info.translation,
                sprite_color: info.sprite_color,
            })
            .collect()
    }

    /// Look up an entity by name.
    pub fn find_by_name(&self, name: &str) -> Option<&SceneEntityNode> {
        self.0.get_entity_by_name(name)
    }

    /// List all entity names in the scene.
    pub fn entity_names(&self) -> Vec<String> {
        self.0.entity_names()
    }

    /// Number of root entities.
    pub fn root_count(&self) -> usize {
        self.0.root_entities.len()
    }

    /// Number of total registered entities.
    pub fn total_count(&self) -> usize {
        self.0.entities_by_name.len()
    }

    /// Incrementally update the index with changed entities.
    /// Uses `Changed<T>` detection to only update entities whose components
    /// actually changed, skipping unchanged entities for performance.
    pub fn incremental_update(
        &mut self,
        adapter: &BevyAdapter,
        all_entities: &Query<(
            Entity,
            Option<&Name>,
            Option<&Transform>,
            Option<&Sprite>,
            Option<&Children>,
            Option<&ChildOf>,
        )>,
        changed: &std::collections::HashSet<Entity>,
        force: bool,
    ) {
        if force {
            // Full rebuild
            self.0 = SceneIndex::new();
        }

        for (entity, name, transform, sprite, children_opt, parent_opt) in all_entities.iter() {
            if !force && !changed.contains(&entity) {
                continue;
            }

            let agent_id = adapter.get_agent_id(entity).map(|id| id.0).unwrap_or(0);

            let entity_name = name
                .map(|n| n.to_string())
                .unwrap_or_else(|| format!("entity_{}", agent_id));

            let mut components: Vec<ComponentSummary> = Vec::new();

            if let Some(t) = transform {
                let mut props = HashMap::new();
                let (roll, pitch, yaw) = t.rotation.to_euler(EulerRot::XYZ);
                props.insert(
                    "translation".into(),
                    serde_json::json!(t.translation.to_array()),
                );
                props.insert("rotation".into(), serde_json::json!([roll, pitch, yaw]));
                props.insert("scale".into(), serde_json::json!(t.scale.to_array()));
                components.push(ComponentSummary {
                    type_name: "Transform".into(),
                    properties: props,
                });
            }

            if sprite.is_some() {
                components.push(ComponentSummary {
                    type_name: "Sprite".into(),
                    properties: HashMap::from([("present".into(), serde_json::json!(true))]),
                });
            }

            if children_opt.is_some() {
                components.push(ComponentSummary {
                    type_name: "Children".into(),
                    properties: HashMap::new(),
                });
            }

            if parent_opt.is_some() {
                components.push(ComponentSummary {
                    type_name: "ChildOf".into(),
                    properties: HashMap::new(),
                });
            }

            self.0.add_entity(entity_name, agent_id, components);
        }

        if !force {
            let live_ids: std::collections::HashSet<u64> = all_entities
                .iter()
                .filter_map(|(e, ..)| adapter.get_agent_id(e).map(|id| id.0))
                .filter(|id| *id != 0)
                .collect();
            self.0.reconcile_deletions(&live_ids);
        }
    }
}

/// Plugin that periodically rebuilds the SceneIndex from Bevy ECS World.
///
/// Usage:
/// ```ignore
/// app.add_plugins(SceneIndexRebuildPlugin::every(30));
/// ```
pub struct SceneIndexRebuildPlugin {
    /// Rebuild interval in frames (default: 30 = ~0.5s at 60fps).
    pub interval_frames: usize,
}

impl SceneIndexRebuildPlugin {
    /// Create a plugin that rebuilds every `interval` frames.
    pub fn every(interval_frames: usize) -> Self {
        Self { interval_frames }
    }
}

impl Default for SceneIndexRebuildPlugin {
    fn default() -> Self {
        Self {
            interval_frames: 30,
        }
    }
}

impl Plugin for SceneIndexRebuildPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SceneIndexCache>()
            .insert_resource(SceneIndexTimer {
                interval: self.interval_frames,
                counter: 0,
                first_run: true,
            })
            .add_systems(Update, rebuild_scene_index_system);
    }
}

/// Internal timer for rebuild interval.
#[derive(Resource)]
struct SceneIndexTimer {
    interval: usize,
    counter: usize,
    first_run: bool,
}

/// System that periodically rebuilds SceneIndex from ECS component queries.
///
/// Queries Name, Transform, Sprite, Children, and Parent components
/// to construct a hierarchical scene graph snapshot.
fn rebuild_scene_index_system(
    mut cache: ResMut<SceneIndexCache>,
    mut timer: ResMut<SceneIndexTimer>,
    adapter: Res<BevyAdapter>,
    query: Query<(
        Entity,
        Option<&Name>,
        Option<&Transform>,
        Option<&Sprite>,
        Option<&Children>,
        Option<&ChildOf>,
    )>,
) {
    timer.counter += 1;
    if !timer.first_run && timer.counter < timer.interval {
        return;
    }
    timer.counter = 0;
    timer.first_run = false;

    // --- Build SceneIndex from queried components ---

    let mut index = SceneIndex::new();
    let mut nodes_map: HashMap<Entity, SceneEntityNode> = HashMap::new();
    let mut roots: Vec<Entity> = Vec::new();
    let mut child_map: HashMap<Entity, Vec<Entity>> = HashMap::new();

    for (entity, name, transform, sprite, children_opt, parent_opt) in query.iter() {
        let agent_id = adapter.get_agent_id(entity).map(|id| id.0).unwrap_or(0);

        let entity_name = name
            .map(|n| n.to_string())
            .unwrap_or_else(|| format!("entity_{}", agent_id));

        let mut components: Vec<ComponentSummary> = Vec::new();

        if let Some(t) = transform {
            let mut props = HashMap::new();
            props.insert(
                "translation".to_string(),
                serde_json::json!([t.translation.x, t.translation.y, t.translation.z]),
            );
            let (roll, pitch, yaw) = t.rotation.to_euler(EulerRot::XYZ);
            props.insert(
                "rotation".to_string(),
                serde_json::json!([roll, pitch, yaw]),
            );
            props.insert(
                "scale".to_string(),
                serde_json::json!([t.scale.x, t.scale.y, t.scale.z]),
            );
            components.push(ComponentSummary {
                type_name: "Transform".to_string(),
                properties: props,
            });
        }

        if let Some(s) = sprite {
            let mut props = HashMap::new();
            let col = s.color.to_linear();
            props.insert(
                "color".to_string(),
                serde_json::json!([col.red, col.green, col.blue, col.alpha]),
            );
            components.push(ComponentSummary {
                type_name: "Sprite".to_string(),
                properties: props,
            });
        }

        let node = SceneEntityNode {
            id: agent_id,
            name: entity_name.clone(),
            components: components.clone(),
            children: Vec::new(),
        };

        for c in &components {
            index
                .entities_by_component
                .entry(c.type_name.clone())
                .or_default()
                .push(agent_id);
        }
        index.entities_by_name.insert(entity_name, agent_id);

        nodes_map.insert(entity, node);

        if parent_opt.is_none() {
            roots.push(entity);
        }

        if let Some(children) = children_opt {
            child_map.insert(entity, children.to_vec());
        }
    }

    // Pass 2: build hierarchy
    fn build_tree(
        entity: Entity,
        nodes_map: &mut HashMap<Entity, SceneEntityNode>,
        child_map: &HashMap<Entity, Vec<Entity>>,
    ) -> SceneEntityNode {
        let mut node = nodes_map.remove(&entity).expect("node must exist");
        if let Some(child_entities) = child_map.get(&entity) {
            for &child_entity in child_entities {
                if nodes_map.contains_key(&child_entity) {
                    let child_node = build_tree(child_entity, nodes_map, child_map);
                    node.children.push(child_node);
                }
            }
        }
        node
    }

    let mut root_entities = Vec::new();
    for root_entity in roots {
        if nodes_map.contains_key(&root_entity) {
            root_entities.push(build_tree(root_entity, &mut nodes_map, &child_map));
        }
    }
    index.root_entities = root_entities;

    cache.0 = index;
}

//! Scene I/O — Bevy World ↔ SceneFile bidirectional conversion
//!
//! CocoIndex-inspired: hash-based incremental save detection, modular component handlers.
//! Supports: Name, Transform, Sprite, Visibility round-trip.

use bevy::prelude::*;
use bevy::sprite::Sprite;
use agent_core::scene_serializer::{SceneFile, SceneEntityData, SerializedComponent, SceneResult};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

fn ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ---------------------------------------------------------------------------
// World → SceneFile
// ---------------------------------------------------------------------------

pub fn scene_from_world(world: &mut World, name: impl Into<String>) -> SceneFile {
    let now = ts();
    let name: String = name.into();
    let mut scene = SceneFile::new(&name);
    scene.created_at = now;
    scene.modified_at = now;

    let mut query = world.query::<(Entity, Option<&Name>, Option<&Transform>, Option<&Sprite>, Option<&Visibility>, Option<&Children>)>();

    let mut entity_map: HashMap<Entity, u64> = HashMap::new();
    let mut next_id: u64 = 1;
    let root_entities: Vec<Entity>;
    let mut child_map: HashMap<Entity, Vec<Entity>> = HashMap::new();
    let mut entities_with_parent: std::collections::HashSet<Entity> = std::collections::HashSet::new();
    let mut all_data: Vec<(Entity, Option<String>, Option<Transform>, Option<Sprite>, Option<Visibility>)> = Vec::new();

    for (entity, name, transform, sprite, visibility, children) in query.iter(world) {
        let name_str = name.map(|n| n.to_string());
        all_data.push((entity, name_str, transform.copied(), sprite.cloned(), visibility.cloned()));

        if let Some(ch) = children {
            for child in ch.iter() {
                entities_with_parent.insert(child);
                child_map.entry(entity).or_default().push(child);
            }
        }
    }

    root_entities = all_data.iter()
        .map(|(e, _, _, _, _)| *e)
        .filter(|e| !entities_with_parent.contains(e))
        .collect();

    for root in &root_entities {
        if let Some(data) = build_entity_data(
            *root, &all_data, &child_map, &mut entity_map, &mut next_id,
        ) {
            scene.add_entity(data);
        }
    }

    scene
}

fn build_entity_data(
    entity: Entity,
    all_data: &[(Entity, Option<String>, Option<Transform>, Option<Sprite>, Option<Visibility>)],
    child_map: &HashMap<Entity, Vec<Entity>>,
    entity_map: &mut HashMap<Entity, u64>,
    next_id: &mut u64,
) -> Option<SceneEntityData> {
    let id = *next_id;
    *next_id += 1;
    entity_map.insert(entity, id);

    let found = all_data.iter().find(|(e, _, _, _, _)| *e == entity)?;
    let (_, name, transform, sprite, visibility) = found;

    let entity_name = name.clone().unwrap_or_else(|| format!("entity_{}", id));
    let mut components: Vec<SerializedComponent> = Vec::new();

    if let Some(t) = transform {
        let (rx, ry, rz) = t.rotation.to_euler(EulerRot::XYZ);
        if let Ok(c) = SerializedComponent::new("Transform", serde_json::json!({
            "translation": [t.translation.x, t.translation.y, t.translation.z],
            "euler_rotation": [rx, ry, rz],
            "rotation_q": [t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w],
            "scale": [t.scale.x, t.scale.y, t.scale.z],
        })) {
            components.push(c);
        }
    }

    if let Some(s) = sprite {
        let c = s.color.to_linear();
        if let Ok(cmp) = SerializedComponent::new("Sprite", serde_json::json!({
            "color_rgba": [c.red, c.green, c.blue, c.alpha],
            "custom_size": s.custom_size.map(|sz| [sz.x, sz.y]),
            "flip_x": s.flip_x,
            "flip_y": s.flip_y,
        })) {
            components.push(cmp);
        }
    }

    if let Some(v) = visibility {
        let vis_str = match v {
            Visibility::Visible => "visible",
            Visibility::Hidden => "hidden",
            Visibility::Inherited => "inherited",
        };
        if let Ok(cmp) = SerializedComponent::new("Visibility", serde_json::json!({ "state": vis_str })) {
            components.push(cmp);
        }
    }

    let children: Vec<SceneEntityData> = child_map.get(&entity)
        .map(|children_list| {
            children_list.iter().filter_map(|child| {
                build_entity_data(*child, all_data, child_map, entity_map, next_id)
            }).collect()
        })
        .unwrap_or_default();

    if components.is_empty() && children.is_empty() {
        return None;
    }

    Some(SceneEntityData {
        id,
        name: entity_name,
        components,
        children,
        parent: None,
    })
}

// ---------------------------------------------------------------------------
// SceneFile → World
// ---------------------------------------------------------------------------

pub fn scene_apply_to_world(world: &mut World, scene: &SceneFile) -> SceneResult<usize> {
    let mut count = 0;
    for entity_data in &scene.entities {
        count += apply_entity(world, entity_data)?;
    }
    Ok(count)
}

fn apply_entity(world: &mut World, data: &SceneEntityData) -> SceneResult<usize> {
    let mut count = 1;
    let entity = world.spawn_empty().id();

    // Name
    if !data.name.is_empty() {
        world.entity_mut(entity).insert(Name::new(data.name.clone()));
    }

    // Components
    for comp in &data.components {
        apply_component(world, entity, comp)?;
    }

    // Children
    for child_data in &data.children {
        let child_count = apply_entity(world, child_data)?;
        // Set parent relationship
        let child_entities: Vec<Entity> = {
            let mut q = world.query::<Entity>();
            let all: Vec<Entity> = q.iter(world).collect();
            all.into_iter().rev().take(child_count).collect()
        };
        for child_entity in child_entities.iter().rev().take(child_count) {
            world.entity_mut(*child_entity).set_parent_in_place(entity);
        }
        count += child_count;
    }

    Ok(count)
}

fn apply_component(world: &mut World, entity: Entity, comp: &SerializedComponent) -> SceneResult<()> {
    match comp.type_name.as_str() {
        "Transform" => {
            let d = &comp.data;
            let tx: f32 = d["translation"].as_array()
                .and_then(|a| a.first()).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let ty: f32 = d["translation"].as_array()
                .and_then(|a| a.get(1)).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let tz: f32 = d["translation"].as_array()
                .and_then(|a| a.get(2)).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;

            let rotation = if let Some(q) = d["rotation_q"].as_array() {
                let q: Vec<f32> = q.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect();
                if q.len() == 4 { Quat::from_xyzw(q[0], q[1], q[2], q[3]) } else { Quat::IDENTITY }
            } else if let Some(e) = d["euler_rotation"].as_array() {
                let e: Vec<f32> = e.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect();
                if e.len() == 3 { Quat::from_euler(EulerRot::XYZ, e[0], e[1], e[2]) } else { Quat::IDENTITY }
            } else { Quat::IDENTITY };

            let sx: f32 = d["scale"].as_array()
                .and_then(|a| a.first()).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
            let sy: f32 = d["scale"].as_array()
                .and_then(|a| a.get(1)).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
            let sz: f32 = d["scale"].as_array()
                .and_then(|a| a.get(2)).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;

            world.entity_mut(entity).insert(Transform {
                translation: Vec3::new(tx, ty, tz),
                rotation,
                scale: Vec3::new(sx, sy, sz),
            });
        }
        "Sprite" => {
            let d = &comp.data;
            let rgba: Vec<f32> = d["color_rgba"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect())
                .unwrap_or_else(|| vec![1.0, 1.0, 1.0, 1.0]);
            let color = Color::linear_rgba(
                *rgba.first().unwrap_or(&1.0),
                *rgba.get(1).unwrap_or(&1.0),
                *rgba.get(2).unwrap_or(&1.0),
                *rgba.get(3).unwrap_or(&1.0),
            );
            let custom_size = d["custom_size"].as_array().map(|a| {
                let v: Vec<f32> = a.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect();
                Vec2::new(*v.first().unwrap_or(&50.0), *v.get(1).unwrap_or(&50.0))
            });
            world.entity_mut(entity).insert(Sprite {
                color,
                custom_size,
                flip_x: d["flip_x"].as_bool().unwrap_or(false),
                flip_y: d["flip_y"].as_bool().unwrap_or(false),
                ..Default::default()
            });
        }
        "Visibility" => {
            let vis = match comp.data["state"].as_str().unwrap_or("visible") {
                "hidden" => Visibility::Hidden,
                "inherited" => Visibility::Inherited,
                _ => Visibility::Visible,
            };
            world.entity_mut(entity).insert(vis);
        }
        "Name" => {
            if let Some(s) = comp.data["value"].as_str() {
                world.entity_mut(entity).insert(Name::new(s.to_string()));
            }
        }
        other => {
            log::warn!("Skipping unsupported component type: {}", other);
        }
    }
    Ok(())
}

/// Content hash for incremental save detection (CocoIndex pattern)
pub fn scene_content_hash(scene: &SceneFile) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    scene.name.hash(&mut h);
    scene.entities.len().hash(&mut h);
    for entity in &scene.entities {
        entity.name.hash(&mut h);
        for comp in &entity.components {
            comp.type_name.hash(&mut h);
        }
    }
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;

    fn make_test_world() -> World {
        let mut world = World::new();
        world.spawn((
            Name::new("Player"),
            Transform::from_xyz(10.0, 20.0, 0.0),
            Sprite { color: Color::linear_rgb(1.0, 0.0, 0.0), custom_size: Some(Vec2::new(32.0, 32.0)), ..Default::default() },
            Visibility::Visible,
        ));
        world.spawn((
            Name::new("Enemy"),
            Transform::from_xyz(100.0, 50.0, 0.0),
            Sprite { color: Color::linear_rgb(0.0, 0.0, 1.0), ..Default::default() },
        ));
        world
    }

    #[test]
    fn test_world_to_scene_and_back() {
        let mut world = make_test_world();
        let scene = scene_from_world(&mut world, "test_level");

        assert_eq!(scene.name, "test_level");
        assert_eq!(scene.entities.len(), 2);

        // Apply to empty world
        let mut empty = World::new();
        let count = scene_apply_to_world(&mut empty, &scene).unwrap();
        assert_eq!(count, 2);

        let names: Vec<String> = empty.query::<&Name>().iter(&empty).map(|n| n.to_string()).collect();
        assert!(names.contains(&"Player".to_string()));
        assert!(names.contains(&"Enemy".to_string()));
    }

    #[test]
    fn test_empty_world() {
        let mut world = World::new();
        let scene = scene_from_world(&mut world, "empty");
        assert!(scene.entities.is_empty());
    }

    #[test]
    fn test_hash_changes() {
        let mut scene1 = SceneFile::new("a");
        let scene2 = SceneFile::new("b");
        assert_ne!(scene_content_hash(&scene1), scene_content_hash(&scene2));

        scene1.add_entity(SceneEntityData {
            id: 1, name: "E".into(), components: vec![], children: vec![], parent: None,
        });
        let scene1_copy = scene1.clone();
        assert_eq!(scene_content_hash(&scene1), scene_content_hash(&scene1_copy));
    }

    #[test]
    fn test_transform_roundtrip() {
        let mut world = World::new();
        world.spawn((
            Name::new("Test"),
            Transform::from_xyz(1.5, 2.5, 3.5),
        ));

        let scene = scene_from_world(&mut world, "t");
        let mut empty = World::new();
        scene_apply_to_world(&mut empty, &scene).unwrap();

        let mut q = empty.query::<&Transform>();
        for t in q.iter(&empty) {
            assert!((t.translation.x - 1.5).abs() < 0.01);
            assert!((t.translation.y - 2.5).abs() < 0.01);
        }
    }

    #[test]
    fn test_visibility_roundtrip() {
        let mut scene = SceneFile::new("v");
        scene.add_entity(SceneEntityData {
            id: 1, name: "Hidden".into(),
            components: vec![SerializedComponent::new("Visibility", serde_json::json!({"state": "hidden"})).unwrap()],
            children: vec![], parent: None,
        });

        let mut world = World::new();
        scene_apply_to_world(&mut world, &scene).unwrap();

        let mut q = world.query::<&Visibility>();
        for v in q.iter(&world) {
            assert_eq!(*v, Visibility::Hidden);
        }
    }

    #[test]
    fn test_scene_save_load() {
        let scene = SceneFile::new("saved");
        let tmp = std::env::temp_dir().join("test_scene_io.json");
        scene.save(&tmp).unwrap();
        let loaded = SceneFile::load(&tmp).unwrap();
        assert_eq!(loaded.name, "saved");
        let _ = std::fs::remove_file(&tmp);
    }
}

//! Prefab Operations — Create from ECS entity, instantiate into World
//!
//! Bridges PrefabDefinition ↔ Bevy ECS using the ComponentPatch format
//! (type_name + HashMap<String, Value> properties).

use bevy::prelude::*;
use bevy::sprite::Sprite;
use agent_core::bevy_editor_model::{PrefabDefinition, PrefabNode, ComponentPatch};
use std::collections::HashMap;

pub fn create_prefab_from_world(
    world: &World,
    entity: Entity,
    name: impl Into<String>,
) -> Option<PrefabDefinition> {
    let name: String = name.into();
    let root = entity_to_prefab_node(world, entity)?;
    Some(PrefabDefinition::new(name.clone(), name, root))
}

fn entity_to_prefab_node(world: &World, entity: Entity) -> Option<PrefabNode> {
    let entity_ref = world.entity(entity);
    let entity_name = entity_ref.get::<Name>()
        .map(|n| n.to_string())
        .unwrap_or_else(|| format!("entity_{:?}", entity));

    let mut node = PrefabNode::new(&entity_name);

    // Transform
    if let Some(t) = entity_ref.get::<Transform>() {
        let mut props = HashMap::new();
        props.insert("translation".into(), serde_json::json!([t.translation.x, t.translation.y, t.translation.z]));
        props.insert("scale".into(), serde_json::json!([t.scale.x, t.scale.y, t.scale.z]));
        props.insert("rotation_q".into(), serde_json::json!([t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w]));
        let (rx, ry, rz) = t.rotation.to_euler(EulerRot::XYZ);
        props.insert("euler".into(), serde_json::json!([rx, ry, rz]));
        node.components.push(ComponentPatch { type_name: "Transform".into(), properties: props });
    }

    // Sprite
    if let Some(s) = entity_ref.get::<Sprite>() {
        let c = s.color.to_linear();
        let mut props = HashMap::new();
        props.insert("color_rgba".into(), serde_json::json!([c.red, c.green, c.blue, c.alpha]));
        if let Some(sz) = s.custom_size {
            props.insert("custom_size".into(), serde_json::json!([sz.x, sz.y]));
        }
        props.insert("flip_x".into(), serde_json::json!(s.flip_x));
        props.insert("flip_y".into(), serde_json::json!(s.flip_y));
        node.components.push(ComponentPatch { type_name: "Sprite".into(), properties: props });
    }

    // Visibility
    if let Some(v) = entity_ref.get::<Visibility>() {
        let mut props = HashMap::new();
        props.insert("state".into(), serde_json::json!(match v {
            Visibility::Visible => "visible",
            Visibility::Hidden => "hidden",
            Visibility::Inherited => "inherited",
        }));
        node.components.push(ComponentPatch { type_name: "Visibility".into(), properties: props });
    }

    // Children recursively
    if let Some(children) = entity_ref.get::<Children>() {
        for child in children.iter() {
            if let Some(child_node) = entity_to_prefab_node(world, child) {
                node.children.push(child_node);
            }
        }
    }

    Some(node)
}

pub fn instantiate_prefab(world: &mut World, prefab: &PrefabDefinition) -> Entity {
    spawn_node(world, &prefab.root)
}

fn spawn_node(world: &mut World, node: &PrefabNode) -> Entity {
    let entity = world.spawn_empty().id();
    if !node.name.is_empty() {
        world.entity_mut(entity).insert(Name::new(node.name.clone()));
    }
    for patch in &node.components {
        apply_patch(world, entity, patch);
    }
    let mut children = Vec::new();
    for child_node in &node.children {
        let child = spawn_node(world, child_node);
        children.push(child);
    }
    for child in children {
        world.entity_mut(child).set_parent_in_place(entity);
    }
    entity
}

fn apply_patch(world: &mut World, entity: Entity, patch: &ComponentPatch) {
    let p = &patch.properties;
    let tn = patch.type_name.as_str();
    match tn {
        "Transform" => {
            let tx = f32_val(p, "translation", 0, 0.0);
            let ty = f32_val(p, "translation", 1, 0.0);
            let tz = f32_val(p, "translation", 2, 0.0);
            let sx = f32_val(p, "scale", 0, 1.0);
            let sy = f32_val(p, "scale", 1, 1.0);
            let sz = f32_val(p, "scale", 2, 1.0);
            let rot = if let Some(serde_json::Value::Array(q)) = p.get("rotation_q") {
                let q: Vec<f32> = q.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect();
                if q.len() == 4 { Quat::from_xyzw(q[0], q[1], q[2], q[3]) } else { Quat::IDENTITY }
            } else if let Some(serde_json::Value::Array(e)) = p.get("euler") {
                let e: Vec<f32> = e.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect();
                if e.len() == 3 { Quat::from_euler(EulerRot::XYZ, e[0], e[1], e[2]) } else { Quat::IDENTITY }
            } else { Quat::IDENTITY };
            world.entity_mut(entity).insert(Transform {
                translation: Vec3::new(tx, ty, tz), rotation: rot, scale: Vec3::new(sx, sy, sz),
            });
        }
        "Sprite" => {
            let rgba = arr_f32(p, "color_rgba", vec![1.0, 1.0, 1.0, 1.0]);
            let sz = p.get("custom_size").and_then(|v| v.as_array()).map(|a| {
                let v: Vec<f32> = a.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect();
                Vec2::new(*v.first().unwrap_or(&50.0), *v.get(1).unwrap_or(&50.0))
            });
            world.entity_mut(entity).insert(Sprite {
                color: Color::linear_rgba(
                    *rgba.first().unwrap_or(&1.0), *rgba.get(1).unwrap_or(&1.0),
                    *rgba.get(2).unwrap_or(&1.0), *rgba.get(3).unwrap_or(&1.0),
                ),
                custom_size: sz,
                flip_x: bool_val(p, "flip_x", false),
                flip_y: bool_val(p, "flip_y", false),
                ..Default::default()
            });
        }
        "Visibility" => {
            let vis = match str_val(p, "state", "visible") {
                "hidden" => Visibility::Hidden,
                "inherited" => Visibility::Inherited,
                _ => Visibility::Visible,
            };
            world.entity_mut(entity).insert(vis);
        }
        _ => log::warn!("Unsupported: {}", tn),
    }
}

fn f32_val(props: &HashMap<String, serde_json::Value>, key: &str, idx: usize, def: f32) -> f32 {
    props.get(key).and_then(|v| v.as_array())
        .and_then(|a| a.get(idx)).and_then(|v| v.as_f64())
        .map(|f| f as f32).unwrap_or(def)
}
fn arr_f32(props: &HashMap<String, serde_json::Value>, key: &str, def: Vec<f32>) -> Vec<f32> {
    props.get(key).and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect())
        .unwrap_or(def)
}
fn bool_val(props: &HashMap<String, serde_json::Value>, key: &str, def: bool) -> bool {
    props.get(key).and_then(|v| v.as_bool()).unwrap_or(def)
}
fn str_val<'a>(props: &'a HashMap<String, serde_json::Value>, key: &str, def: &'a str) -> &'a str {
    props.get(key).and_then(|v| v.as_str()).unwrap_or(def)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_prefab() {
        let mut world = World::new();
        let root = world.spawn((
            Name::new("TestPrefab"),
            Transform::from_xyz(10.0, 20.0, 0.0),
            Sprite { color: Color::linear_rgb(1.0, 0.0, 0.0), custom_size: Some(Vec2::new(32.0, 32.0)), ..Default::default() },
        )).id();

        let prefab = create_prefab_from_world(&world, root, "MyPrefab").unwrap();
        assert_eq!(prefab.root.name, "TestPrefab");
        assert_eq!(prefab.root.components.len(), 3); // Transform + Sprite + Visibility(auto)
    }

    #[test]
    fn test_instantiate() {
        let mut world = World::new();
        let src = world.spawn((Name::new("Src"), Transform::from_xyz(1.0, 2.0, 0.0))).id();
        let prefab = create_prefab_from_world(&world, src, "CloneSrc").unwrap();
        let instance = instantiate_prefab(&mut world, &prefab);
        assert!(world.entity(instance).contains::<Transform>());
    }

    #[test]
    fn test_roundtrip() {
        let mut world = World::new();
        let parent = world.spawn((Name::new("P"), Transform::from_xyz(0.0, 0.0, 0.0))).id();
        let child = world.spawn((Name::new("C"), Transform::from_xyz(10.0, 0.0, 0.0))).id();
        world.entity_mut(child).set_parent_in_place(parent);

        let prefab = create_prefab_from_world(&world, parent, "Hierarchy").unwrap();
        assert_eq!(prefab.root.children.len(), 1);

        let mut empty = World::new();
        instantiate_prefab(&mut empty, &prefab);
        let names: Vec<String> = empty.query::<&Name>().iter(&empty).map(|n| n.to_string()).collect();
        assert!(names.contains(&"P".to_string()) && names.contains(&"C".to_string()));
    }
}

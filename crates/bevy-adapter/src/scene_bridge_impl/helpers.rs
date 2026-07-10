use super::commands::{SceneCommand, SceneCommandQueue};

// ---------------------------------------------------------------------------
// Convenience Functions
// ---------------------------------------------------------------------------

/// Request entity creation via command queue
pub fn request_create_entity(
    queue: &mut SceneCommandQueue,
    name: &str,
    position: Option<[f64; 2]>,
) -> u64 {
    queue.push(SceneCommand::CreateEntity {
        name: name.to_string(),
        position,
        components: Vec::new(),
    })
}

/// Request prefab instantiation via command queue
pub fn request_instantiate_prefab(
    queue: &mut SceneCommandQueue,
    prefab_path: &str,
    position: [f64; 3],
) -> u64 {
    queue.push(SceneCommand::InstantiatePrefab {
        prefab_path: prefab_path.to_string(),
        position: Some(position),
        rotation: None,
        scale: None,
    })
}

/// Request scene save
pub fn request_save_scene(queue: &mut SceneCommandQueue, path: &str) -> u64 {
    queue.push(SceneCommand::SaveScene {
        path: path.to_string(),
    })
}

/// Request scene load
pub fn request_load_scene(queue: &mut SceneCommandQueue, path: &str) -> u64 {
    queue.push(SceneCommand::LoadScene {
        path: path.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Helper Functions
// ---------------------------------------------------------------------------

pub fn parse_color_array(value: &serde_json::Value) -> [f32; 4] {
    if let Some(arr) = value.as_array() {
        let r = arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
        let g = arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
        let b = arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
        let a = arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
        [r, g, b, a]
    } else {
        [1.0, 1.0, 1.0, 1.0]
    }
}

use agent_core::application::{DirectorRuntime, EditorEvent};
use agent_core::ports::scene::{
    create_shared_bridge, ComponentPatch, EntityListItem, MockSceneBridge, SceneBridge,
};

fn assert_scene_bridge<T: SceneBridge>() {}

#[test]
fn stable_facades_export_existing_contracts() {
    assert_scene_bridge::<MockSceneBridge>();
    let _runtime = DirectorRuntime::new();
    let _bridge = create_shared_bridge(Box::new(MockSceneBridge::new()));
    let _: Option<ComponentPatch> = None;
    let _: Option<EntityListItem> = None;
    let _: Option<EditorEvent> = None;
}

//! Editor Selection System
//!
//! Provides a centralized selection state that can be accessed by:
//! - UI panels (Hierarchy, Inspector)
//! - Agent systems for context-aware AI decisions
//! - Shortcut handlers for entity operations

use bevy::prelude::*;

pub struct EditorSelectionPlugin;

impl Plugin for EditorSelectionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorSelection>()
            .add_systems(Update, sync_selection_from_panels)
            .add_systems(Update, broadcast_selection_change);
    }
}

/// Centralized editor selection state
/// This is the single source of truth for entity selection across all editor panels
#[derive(Resource, Default, Debug, Clone)]
pub struct EditorSelection {
    /// Currently selected entity (single selection for now)
    pub selected_entity: Option<Entity>,
    /// Previous selection for undo support
    pub previous_selection: Option<Entity>,
    /// Selection change timestamp
    pub last_changed: f64,
    /// Additional context about the selection
    pub context: SelectionContext,
}

/// Context information about the selected entity
#[derive(Default, Debug, Clone)]
pub struct SelectionContext {
    /// Entity name if available
    pub entity_name: Option<String>,
    /// Whether entity has RuntimeAgentComponent
    pub has_agent: bool,
    /// Whether entity has Transform
    pub has_transform: bool,
    /// Whether entity has Sprite/Mesh
    pub has_visual: bool,
    /// Component types attached to entity
    pub component_types: Vec<String>,
}

/// Event sent when selection changes
#[derive(Message, Debug, Clone)]
pub struct SelectionChangedEvent {
    pub old_entity: Option<Entity>,
    pub new_entity: Option<Entity>,
    pub context: SelectionContext,
}

/// Sync selection from HierarchyState and InspectorState to EditorSelection
fn sync_selection_from_panels(
    hierarchy_state: Res<crate::hierarchy_panel::HierarchyState>,
    mut editor_selection: ResMut<EditorSelection>,
    time: Res<Time>,
) {
    // Hierarchy panel now writes directly to EditorSelection via select().
    // This system provides a fallback sync if other panels set HierarchyState.selected_entity.
    let new_selection = hierarchy_state.selected_entity;

    if editor_selection.selected_entity != new_selection {
        editor_selection.previous_selection = editor_selection.selected_entity;
        editor_selection.selected_entity = new_selection;
        editor_selection.last_changed = time.elapsed_secs_f64();
    }
}

/// Broadcast selection changes to interested systems and update context
fn broadcast_selection_change(
    mut editor_selection: ResMut<EditorSelection>,
    mut events: MessageWriter<SelectionChangedEvent>,
    name_query: Query<&Name>,
    agent_query: Query<&bevy_adapter::RuntimeAgentComponent>,
    transform_query: Query<&Transform>,
    sprite_query: Query<&Sprite>,
) {
    // Only process if we have a selection and need to update context
    let Some(entity) = editor_selection.selected_entity else {
        return;
    };

    // Update context information
    let mut context = SelectionContext::default();

    // Check entity name
    if let Ok(name) = name_query.get(entity) {
        context.entity_name = Some(name.as_str().to_string());
    }

    // Check components
    context.has_agent = agent_query.get(entity).is_ok();
    context.has_transform = transform_query.get(entity).is_ok();
    context.has_visual = sprite_query.get(entity).is_ok();

    // Build component type list
    context.component_types = vec![];
    if context.has_agent {
        context.component_types.push("RuntimeAgent".to_string());
    }
    if context.has_transform {
        context.component_types.push("Transform".to_string());
    }
    if context.has_visual {
        context.component_types.push("Sprite".to_string());
    }

    // Check if context changed significantly
    let context_changed = editor_selection.context.entity_name != context.entity_name
        || editor_selection.context.has_agent != context.has_agent
        || editor_selection.context.has_transform != context.has_transform;

    if context_changed {
        let old_context = editor_selection.context.clone();
        let component_types = context.component_types.clone();
        editor_selection.context = context.clone();

        // Send event
        events.write(SelectionChangedEvent {
            old_entity: editor_selection.previous_selection,
            new_entity: Some(entity),
            context,
        });

        info!(
            "Selection changed: {:?} -> {:?} (name: {:?}, components: {:?})",
            editor_selection.previous_selection,
            entity,
            old_context.entity_name,
            component_types
        );
    }
}

/// Helper functions for selection manipulation
impl EditorSelection {
    /// Clear the current selection
    pub fn clear(&mut self) {
        self.previous_selection = self.selected_entity;
        self.selected_entity = None;
        self.context = SelectionContext::default();
    }

    /// Select a specific entity
    pub fn select(&mut self, entity: Entity) {
        self.previous_selection = self.selected_entity;
        self.selected_entity = Some(entity);
    }

    /// Select with timestamp for change tracking
    pub fn select_with_time(&mut self, entity: Entity, time: f64) {
        self.previous_selection = self.selected_entity;
        self.selected_entity = Some(entity);
        self.last_changed = time;
    }

    /// Check if an entity is currently selected
    pub fn is_selected(&self, entity: Entity) -> bool {
        self.selected_entity == Some(entity)
    }

    /// Get a description of the current selection for display
    pub fn description(&self) -> String {
        match (&self.selected_entity, &self.context.entity_name) {
            (Some(entity), Some(name)) => format!("{} ({:?})", name, entity),
            (Some(entity), None) => format!("Entity {:?}", entity),
            (None, _) => "No selection".to_string(),
        }
    }

    /// Get selection context as a compact string for Agent prompts
    pub fn to_agent_context(&self) -> String {
        match &self.selected_entity {
            Some(entity) => {
                let name = self.context.entity_name.as_deref().unwrap_or("unnamed");
                let components = if self.context.component_types.is_empty() {
                    "no components".to_string()
                } else {
                    self.context.component_types.join(", ")
                };
                format!(
                    "Selected entity: {} ({:?}) with components: {}",
                    name, entity, components
                )
            }
            None => "No entity currently selected".to_string(),
        }
    }
}

/// System to handle selection shortcuts (Esc to clear, etc.)
pub fn selection_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    mut editor_selection: ResMut<EditorSelection>,
) {
    // Escape to clear selection
    if keys.just_pressed(KeyCode::Escape) {
        if editor_selection.selected_entity.is_some() {
            info!("Clearing selection (Escape pressed)");
            editor_selection.clear();
        }
    }
}

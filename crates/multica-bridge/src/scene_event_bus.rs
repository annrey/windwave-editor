//! Scene Event Bus
//!
//! Event bus for scene changes, allowing subscribers to receive notifications
//! when entities are created, updated, or deleted.

use crate::scene_context::{ComponentData, SceneEntity};
use log::info;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Type of scene event
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneEventType {
    /// Entity was created
    EntityCreated,
    /// Entity was updated
    EntityUpdated,
    /// Entity was deleted
    EntityDeleted,
    /// Component was added to an entity
    ComponentAdded,
    /// Component was updated on an entity
    ComponentUpdated,
    /// Component was removed from an entity
    ComponentRemoved,
}

/// Scene event containing all relevant information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneEvent {
    /// ID of the scene this event belongs to
    pub scene_id: String,
    /// Type of the event
    pub event_type: SceneEventType,
    /// ID of the affected entity
    pub entity_id: u64,
    /// Entity data (before the change if applicable)
    pub entity_before: Option<SceneEntity>,
    /// Entity data (after the change if applicable)
    pub entity_after: Option<SceneEntity>,
    /// Component data if applicable
    pub component: Option<ComponentData>,
    /// Timestamp of the event
    pub timestamp: String,
}

/// Trait for scene event subscribers
pub trait SceneEventSubscriber: Send + Sync {
    /// Handle a scene event
    fn on_event(&self, event: &SceneEvent);
}

/// Type alias for subscriber ID
pub type SubscriberId = u64;

/// Scene event bus
pub struct SceneEventBus {
    /// Next subscriber ID to assign
    next_subscriber_id: SubscriberId,
    /// Subscribers mapped by ID
    subscribers: HashMap<SubscriberId, Arc<dyn SceneEventSubscriber>>,
    /// History of events
    event_history: Vec<SceneEvent>,
    /// Maximum history size
    max_history_size: usize,
}

impl SceneEventBus {
    /// Create a new scene event bus
    pub fn new() -> Self {
        Self {
            next_subscriber_id: 1,
            subscribers: HashMap::new(),
            event_history: Vec::new(),
            max_history_size: 1000,
        }
    }

    /// Create a new scene event bus with custom max history size
    pub fn with_max_history(max_history_size: usize) -> Self {
        Self {
            next_subscriber_id: 1,
            subscribers: HashMap::new(),
            event_history: Vec::new(),
            max_history_size,
        }
    }

    /// Subscribe to scene events
    pub fn subscribe(&mut self, subscriber: Arc<dyn SceneEventSubscriber>) -> SubscriberId {
        let id = self.next_subscriber_id;
        self.next_subscriber_id += 1;
        info!("Subscribed scene event listener with ID: {}", id);
        self.subscribers.insert(id, subscriber);
        id
    }

    /// Unsubscribe from scene events
    pub fn unsubscribe(&mut self, subscriber_id: SubscriberId) {
        if self.subscribers.remove(&subscriber_id).is_some() {
            info!(
                "Unsubscribed scene event listener with ID: {}",
                subscriber_id
            );
        }
    }

    /// Publish an event
    pub fn publish(&mut self, event: SceneEvent) {
        info!(
            "Publishing scene event: {:?} for entity {}",
            event.event_type, event.entity_id
        );

        // Add to history
        self.event_history.push(event.clone());

        // Trim history if needed
        if self.event_history.len() > self.max_history_size {
            self.event_history.remove(0);
        }

        // Notify all subscribers
        for subscriber in self.subscribers.values() {
            subscriber.on_event(&event);
        }
    }

    /// Helper method to publish entity created event
    pub fn publish_entity_created(&mut self, scene_id: String, entity: SceneEntity) {
        let event = SceneEvent {
            scene_id,
            event_type: SceneEventType::EntityCreated,
            entity_id: entity.id,
            entity_before: None,
            entity_after: Some(entity),
            component: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        self.publish(event);
    }

    /// Helper method to publish entity updated event
    pub fn publish_entity_updated(
        &mut self,
        scene_id: String,
        entity_before: SceneEntity,
        entity_after: SceneEntity,
    ) {
        let event = SceneEvent {
            scene_id,
            event_type: SceneEventType::EntityUpdated,
            entity_id: entity_after.id,
            entity_before: Some(entity_before),
            entity_after: Some(entity_after),
            component: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        self.publish(event);
    }

    /// Helper method to publish entity deleted event
    pub fn publish_entity_deleted(&mut self, scene_id: String, entity: SceneEntity) {
        let event = SceneEvent {
            scene_id,
            event_type: SceneEventType::EntityDeleted,
            entity_id: entity.id,
            entity_before: Some(entity),
            entity_after: None,
            component: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        self.publish(event);
    }

    /// Helper method to publish component added event
    pub fn publish_component_added(
        &mut self,
        scene_id: String,
        entity_id: u64,
        component: ComponentData,
    ) {
        let event = SceneEvent {
            scene_id,
            event_type: SceneEventType::ComponentAdded,
            entity_id,
            entity_before: None,
            entity_after: None,
            component: Some(component),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        self.publish(event);
    }

    /// Helper method to publish component updated event
    pub fn publish_component_updated(
        &mut self,
        scene_id: String,
        entity_id: u64,
        component: ComponentData,
    ) {
        let event = SceneEvent {
            scene_id,
            event_type: SceneEventType::ComponentUpdated,
            entity_id,
            entity_before: None,
            entity_after: None,
            component: Some(component),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        self.publish(event);
    }

    /// Get event history
    pub fn event_history(&self) -> &[SceneEvent] {
        &self.event_history
    }

    /// Get recent events
    pub fn recent_events(&self, count: usize) -> &[SceneEvent] {
        let start = self.event_history.len().saturating_sub(count);
        &self.event_history[start..]
    }

    /// Get number of subscribers
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }
}

impl Default for SceneEventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe shared scene event bus
pub type SharedSceneEventBus = Arc<Mutex<SceneEventBus>>;

/// Create a shared scene event bus
pub fn create_shared_event_bus() -> SharedSceneEventBus {
    Arc::new(Mutex::new(SceneEventBus::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestSubscriber {
        event_count: AtomicUsize,
    }

    impl TestSubscriber {
        fn new() -> Self {
            Self {
                event_count: AtomicUsize::new(0),
            }
        }
    }

    impl SceneEventSubscriber for TestSubscriber {
        fn on_event(&self, _event: &SceneEvent) {
            self.event_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn test_publish_and_subscribe() {
        let mut bus = SceneEventBus::new();

        let subscriber = Arc::new(TestSubscriber::new());
        let id = bus.subscribe(subscriber.clone());

        let entity = SceneEntity {
            id: 1,
            name: "TestEntity".to_string(),
            components: vec![],
            position: None,
        };

        bus.publish_entity_created("scene-1".to_string(), entity);

        assert_eq!(subscriber.event_count.load(Ordering::SeqCst), 1);

        bus.unsubscribe(id);
    }

    #[test]
    fn test_event_history() {
        let mut bus = SceneEventBus::with_max_history(5);

        for i in 1..=10 {
            let entity = SceneEntity {
                id: i,
                name: format!("Entity{}", i),
                components: vec![],
                position: None,
            };
            bus.publish_entity_created("scene-1".to_string(), entity);
        }

        assert_eq!(bus.event_history().len(), 5);
    }

    #[test]
    fn test_multiple_subscribers() {
        let mut bus = SceneEventBus::new();

        let sub1 = Arc::new(TestSubscriber::new());
        let sub2 = Arc::new(TestSubscriber::new());

        let _id1 = bus.subscribe(sub1.clone());
        let _id2 = bus.subscribe(sub2.clone());

        let entity = SceneEntity {
            id: 1,
            name: "TestEntity".to_string(),
            components: vec![],
            position: None,
        };

        bus.publish_entity_created("scene-1".to_string(), entity);

        assert_eq!(sub1.event_count.load(Ordering::SeqCst), 1);
        assert_eq!(sub2.event_count.load(Ordering::SeqCst), 1);
    }
}

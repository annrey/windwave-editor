//! Gameplay primitive catalog for AI-generated playable slices.
//!
//! The catalog defines what gameplay building blocks an AI plan may reference.
//! Runtime behavior and Bevy component mapping live elsewhere; this module is
//! the stable capability boundary used by OpenWorldPlan validation.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum GameplayPrimitiveKind {
    PlayerController,
    FollowCamera,
    Interactable,
    InteractionZone,
    Quest,
    QuestObjective,
    PuzzleSwitch,
    LootContainer,
    Inventory,
    Combatant,
    Attack,
    EnemyBrain,
    WorldSurface,
    ZoneMarker,
}

impl GameplayPrimitiveKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::PlayerController => "PlayerController",
            Self::FollowCamera => "FollowCamera",
            Self::Interactable => "Interactable",
            Self::InteractionZone => "InteractionZone",
            Self::Quest => "Quest",
            Self::QuestObjective => "QuestObjective",
            Self::PuzzleSwitch => "PuzzleSwitch",
            Self::LootContainer => "LootContainer",
            Self::Inventory => "Inventory",
            Self::Combatant => "Combatant",
            Self::Attack => "Attack",
            Self::EnemyBrain => "EnemyBrain",
            Self::WorldSurface => "WorldSurface",
            Self::ZoneMarker => "ZoneMarker",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum GameplayPrimitiveCategory {
    Player,
    Camera,
    Interaction,
    Quest,
    Puzzle,
    Loot,
    Combat,
    World,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum GameplayCapability {
    SceneAuthoring,
    RuntimeControl,
    RuntimeState,
    UiFeedback,
    Verification,
    Persistence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameplayPrimitiveDescriptor {
    pub kind: GameplayPrimitiveKind,
    pub id: String,
    pub category: GameplayPrimitiveCategory,
    pub summary: String,
    pub minimum_behavior: Vec<String>,
    pub capabilities: Vec<GameplayCapability>,
    pub dependencies: Vec<GameplayPrimitiveKind>,
}

impl GameplayPrimitiveDescriptor {
    pub fn new(
        kind: GameplayPrimitiveKind,
        category: GameplayPrimitiveCategory,
        summary: impl Into<String>,
        minimum_behavior: Vec<&str>,
        capabilities: Vec<GameplayCapability>,
        dependencies: Vec<GameplayPrimitiveKind>,
    ) -> Self {
        Self {
            kind,
            id: kind.id().to_string(),
            category,
            summary: summary.into(),
            minimum_behavior: minimum_behavior.into_iter().map(str::to_string).collect(),
            capabilities,
            dependencies,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameplayPrimitiveCatalog {
    primitives: BTreeMap<GameplayPrimitiveKind, GameplayPrimitiveDescriptor>,
}

impl GameplayPrimitiveCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_world_slice_defaults() -> Self {
        use GameplayCapability::*;

        let mut catalog = Self::new();
        for descriptor in [
            GameplayPrimitiveDescriptor::new(
                GameplayPrimitiveKind::PlayerController,
                GameplayPrimitiveCategory::Player,
                "Moves the playable avatar through the world.",
                vec!["movement", "facing", "speed limit"],
                vec![RuntimeControl, RuntimeState, Verification],
                vec![GameplayPrimitiveKind::WorldSurface],
            ),
            GameplayPrimitiveDescriptor::new(
                GameplayPrimitiveKind::FollowCamera,
                GameplayPrimitiveCategory::Camera,
                "Keeps the camera attached to the player with a stable offset.",
                vec!["target follow", "offset", "stable update"],
                vec![RuntimeControl, Verification],
                vec![GameplayPrimitiveKind::PlayerController],
            ),
            GameplayPrimitiveDescriptor::new(
                GameplayPrimitiveKind::Interactable,
                GameplayPrimitiveCategory::Interaction,
                "Allows a player or scripted test to trigger an object.",
                vec!["enter range", "trigger interaction", "emit event"],
                vec![RuntimeState, UiFeedback, Verification],
                vec![GameplayPrimitiveKind::InteractionZone],
            ),
            GameplayPrimitiveDescriptor::new(
                GameplayPrimitiveKind::InteractionZone,
                GameplayPrimitiveCategory::Interaction,
                "Defines where an object can be interacted with.",
                vec!["range check", "target binding"],
                vec![SceneAuthoring, RuntimeState, Verification],
                vec![],
            ),
            GameplayPrimitiveDescriptor::new(
                GameplayPrimitiveKind::Quest,
                GameplayPrimitiveCategory::Quest,
                "Tracks a playable objective chain.",
                vec!["state", "objective list", "completion check"],
                vec![RuntimeState, UiFeedback, Verification, Persistence],
                vec![GameplayPrimitiveKind::QuestObjective],
            ),
            GameplayPrimitiveDescriptor::new(
                GameplayPrimitiveKind::QuestObjective,
                GameplayPrimitiveCategory::Quest,
                "Tracks one objective inside a quest.",
                vec!["objective kind", "target id", "completion state"],
                vec![RuntimeState, UiFeedback, Verification, Persistence],
                vec![],
            ),
            GameplayPrimitiveDescriptor::new(
                GameplayPrimitiveKind::PuzzleSwitch,
                GameplayPrimitiveCategory::Puzzle,
                "Represents a simple switch-driven puzzle state machine.",
                vec!["locked", "awaiting interaction", "solved"],
                vec![RuntimeState, Verification, Persistence],
                vec![GameplayPrimitiveKind::Interactable],
            ),
            GameplayPrimitiveDescriptor::new(
                GameplayPrimitiveKind::LootContainer,
                GameplayPrimitiveCategory::Loot,
                "Stores and releases rewards after unlock conditions are met.",
                vec!["locked", "unlocked", "opened", "loot claimed"],
                vec![RuntimeState, UiFeedback, Verification, Persistence],
                vec![GameplayPrimitiveKind::Inventory],
            ),
            GameplayPrimitiveDescriptor::new(
                GameplayPrimitiveKind::Inventory,
                GameplayPrimitiveCategory::Loot,
                "Stores collected rewards for the player.",
                vec!["add item", "contains item", "prevent duplicates"],
                vec![RuntimeState, UiFeedback, Verification, Persistence],
                vec![],
            ),
            GameplayPrimitiveDescriptor::new(
                GameplayPrimitiveKind::Combatant,
                GameplayPrimitiveCategory::Combat,
                "Defines health, faction, and defeated state for combat actors.",
                vec!["health", "faction", "defeated state"],
                vec![RuntimeState, Verification, Persistence],
                vec![],
            ),
            GameplayPrimitiveDescriptor::new(
                GameplayPrimitiveKind::Attack,
                GameplayPrimitiveCategory::Combat,
                "Applies damage from an attacker to a combat target.",
                vec!["damage", "cooldown", "target"],
                vec![RuntimeControl, RuntimeState, Verification],
                vec![GameplayPrimitiveKind::Combatant],
            ),
            GameplayPrimitiveDescriptor::new(
                GameplayPrimitiveKind::EnemyBrain,
                GameplayPrimitiveCategory::Combat,
                "Controls minimal enemy idle, patrol, aggro, attack, and dead states.",
                vec!["idle", "patrol", "aggro", "attack", "dead"],
                vec![RuntimeControl, RuntimeState, Verification],
                vec![
                    GameplayPrimitiveKind::Combatant,
                    GameplayPrimitiveKind::ZoneMarker,
                ],
            ),
            GameplayPrimitiveDescriptor::new(
                GameplayPrimitiveKind::WorldSurface,
                GameplayPrimitiveCategory::World,
                "Marks terrain or surfaces that can be traversed.",
                vec!["walkable area", "queryable bounds"],
                vec![SceneAuthoring, Verification],
                vec![],
            ),
            GameplayPrimitiveDescriptor::new(
                GameplayPrimitiveKind::ZoneMarker,
                GameplayPrimitiveCategory::World,
                "Names logical world areas for plans, SceneIndex queries, and tests.",
                vec!["stable id", "bounds", "semantic label"],
                vec![SceneAuthoring, Verification],
                vec![],
            ),
        ] {
            catalog.register(descriptor);
        }
        catalog
    }

    pub fn register(&mut self, descriptor: GameplayPrimitiveDescriptor) {
        self.primitives.insert(descriptor.kind, descriptor);
    }

    pub fn get(&self, kind: GameplayPrimitiveKind) -> Option<&GameplayPrimitiveDescriptor> {
        self.primitives.get(&kind)
    }

    pub fn contains(&self, kind: GameplayPrimitiveKind) -> bool {
        self.primitives.contains_key(&kind)
    }

    pub fn list(&self) -> Vec<&GameplayPrimitiveDescriptor> {
        self.primitives.values().collect()
    }

    pub fn ids(&self) -> Vec<&str> {
        self.primitives
            .values()
            .map(|descriptor| descriptor.id.as_str())
            .collect()
    }

    pub fn validate_references(
        &self,
        required: &[GameplayPrimitiveKind],
    ) -> Result<(), GameplayPrimitiveValidationError> {
        let missing: Vec<GameplayPrimitiveKind> = required
            .iter()
            .copied()
            .filter(|kind| !self.contains(*kind))
            .collect();

        if missing.is_empty() {
            Ok(())
        } else {
            Err(GameplayPrimitiveValidationError { missing })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameplayPrimitiveValidationError {
    pub missing: Vec<GameplayPrimitiveKind>,
}

impl std::fmt::Display for GameplayPrimitiveValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ids: Vec<&str> = self.missing.iter().map(|kind| kind.id()).collect();
        write!(f, "missing gameplay primitives: {}", ids.join(", "))
    }
}

impl std::error::Error for GameplayPrimitiveValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_catalog_contains_open_world_slice_primitives() {
        let catalog = GameplayPrimitiveCatalog::open_world_slice_defaults();
        let required = [
            GameplayPrimitiveKind::PlayerController,
            GameplayPrimitiveKind::FollowCamera,
            GameplayPrimitiveKind::Interactable,
            GameplayPrimitiveKind::InteractionZone,
            GameplayPrimitiveKind::Quest,
            GameplayPrimitiveKind::QuestObjective,
            GameplayPrimitiveKind::PuzzleSwitch,
            GameplayPrimitiveKind::LootContainer,
            GameplayPrimitiveKind::Inventory,
            GameplayPrimitiveKind::Combatant,
            GameplayPrimitiveKind::Attack,
            GameplayPrimitiveKind::EnemyBrain,
            GameplayPrimitiveKind::WorldSurface,
            GameplayPrimitiveKind::ZoneMarker,
        ];

        assert_eq!(catalog.list().len(), required.len());
        catalog.validate_references(&required).unwrap();
    }

    #[test]
    fn default_catalog_describes_dependencies() {
        let catalog = GameplayPrimitiveCatalog::open_world_slice_defaults();
        let enemy_brain = catalog
            .get(GameplayPrimitiveKind::EnemyBrain)
            .expect("EnemyBrain should be registered");

        assert!(enemy_brain
            .dependencies
            .contains(&GameplayPrimitiveKind::Combatant));
        assert!(enemy_brain
            .dependencies
            .contains(&GameplayPrimitiveKind::ZoneMarker));
    }

    #[test]
    fn validate_references_reports_missing_primitives() {
        let catalog = GameplayPrimitiveCatalog::new();
        let error = catalog
            .validate_references(&[GameplayPrimitiveKind::PlayerController])
            .expect_err("empty catalog should reject required primitives");

        assert_eq!(error.missing, vec![GameplayPrimitiveKind::PlayerController]);
        assert_eq!(
            error.to_string(),
            "missing gameplay primitives: PlayerController"
        );
    }

    #[test]
    fn catalog_round_trips_through_json() {
        let catalog = GameplayPrimitiveCatalog::open_world_slice_defaults();
        let json = serde_json::to_string(&catalog).unwrap();
        let decoded: GameplayPrimitiveCatalog = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded, catalog);
        assert!(decoded
            .ids()
            .contains(&GameplayPrimitiveKind::PuzzleSwitch.id()));
    }
}

//! 玩家控制器示例
//!
//! 展示如何使用游戏输入系统控制玩家角色

use bevy::prelude::*;
use bevy::sprite::Sprite;
use log::info;

use crate::game_input::{GameInputAction, GameInputEvent, InputPhase, InputStateBuffer};
use crate::interactable::{
    cancel_interaction, start_interaction, InteractionCancelEvent, InteractionCompleteEvent,
    InteractionStartEvent, PlayerInteractionState,
};
use crate::play_mode::PlayModeState;

/// 玩家标记组件
#[derive(Component, Default)]
pub struct Player;

/// 玩家移动速度
#[derive(Resource)]
pub struct PlayerSpeed(pub f32);

impl Default for PlayerSpeed {
    fn default() -> Self {
        Self(200.0)
    }
}

/// 玩家控制器插件
pub struct PlayerControllerPlugin;

impl Plugin for PlayerControllerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerSpeed>().add_systems(
            Update,
            (
                player_movement.run_if(|state: Res<PlayModeState>| state.is_playing),
                handle_interaction_input.run_if(|state: Res<PlayModeState>| state.is_playing),
                handle_jump_input.run_if(|state: Res<PlayModeState>| state.is_playing),
            ),
        );
    }
}

/// 处理玩家移动
fn player_movement(
    time: Res<Time>,
    input_state: Res<InputStateBuffer>,
    speed: Res<PlayerSpeed>,
    mut query: Query<&mut Transform, With<Player>>,
) {
    let dir = input_state.move_direction();
    if dir.length_squared() > 0.0 {
        for mut transform in query.iter_mut() {
            let delta = dir * speed.0 * time.delta_secs();
            transform.translation.x += delta.x;
            transform.translation.y += delta.y;
        }
    }
}

/// 处理跳跃输入
fn handle_jump_input(
    mut events: Option<MessageReader<GameInputEvent>>,
    mut query: Query<&mut Transform, With<Player>>,
) {
    for event in events.iter_mut().flat_map(|r| r.read()) {
        if let (GameInputAction::Jump, InputPhase::Pressed) = (&event.action, event.phase) {
            info!("玩家跳跃！");
            // 这里可以添加跳跃逻辑
            for mut transform in query.iter_mut() {
                transform.translation.z += 10.0; // 简单的"跳跃"效果
            }
        }
    }
}

/// 处理交互输入
fn handle_interaction_input(
    mut commands: Commands,
    mut events: Option<MessageReader<GameInputEvent>>,
    player_query: Query<Entity, With<Player>>,
    mut player_state: ResMut<PlayerInteractionState>,
    mut progress: ResMut<crate::interactable::InteractionProgress>,
    mut interactable_states: Query<&mut crate::interactable::InteractableState>,
    interactable_types: Query<&crate::interactable::InteractionType>,
    mut start_events: MessageWriter<InteractionStartEvent>,
    mut cancel_events: MessageWriter<InteractionCancelEvent>,
) {
    let Ok(player) = player_query.single() else {
        return;
    };

    for event in events.iter_mut().flat_map(|r| r.read()) {
        match (&event.action, event.phase) {
            (GameInputAction::Interact, InputPhase::Pressed) => {
                if let PlayerInteractionState::Hovering(target) = *player_state {
                    // 开始交互
                    if let Ok(interaction_type) = interactable_types.get(target) {
                        start_interaction(
                            &mut commands,
                            player,
                            target,
                            *interaction_type,
                            &mut progress,
                            &mut player_state,
                            &mut interactable_states,
                            &mut start_events,
                        );
                    }
                }
            }
            (GameInputAction::Interact, InputPhase::Released) => {
                if let PlayerInteractionState::Interacting(target) = *player_state {
                    // 取消交互
                    cancel_interaction(
                        target,
                        &mut progress,
                        &mut player_state,
                        &mut interactable_states,
                        &mut cancel_events,
                        player,
                    );
                }
            }
            _ => {}
        }
    }
}

/// 监听交互完成事件
pub fn handle_interaction_complete(mut events: Option<MessageReader<InteractionCompleteEvent>>) {
    for event in events.iter_mut().flat_map(|r| r.read()) {
        info!(
            "交互完成！类型: {:?}, 成功: {}",
            event.interaction_type, event.success
        );
        // 这里可以添加交互完成后的逻辑，如拾取物品、触发对话等
    }
}

/// 生成一个玩家实体的便捷函数
pub fn spawn_player(commands: &mut Commands, position: Vec3, color: Color) -> Entity {
    commands
        .spawn((
            Sprite {
                custom_size: Some(Vec2::new(32.0, 32.0)),
                color,
                ..default()
            },
            Transform::from_translation(position),
            GlobalTransform::default(),
            Player,
            Name::new("Player"),
        ))
        .id()
}

/// 生成一个可交互物体的便捷函数
pub fn spawn_interactable(
    commands: &mut Commands,
    position: Vec3,
    color: Color,
    interaction_type: crate::interactable::InteractionType,
    prompt_text: &str,
) -> Entity {
    commands
        .spawn((
            Sprite {
                custom_size: Some(Vec2::new(32.0, 32.0)),
                color,
                ..default()
            },
            Transform::from_translation(position),
            GlobalTransform::default(),
            crate::interactable::Interactable,
            interaction_type,
            crate::interactable::InteractionRadius::default(),
            crate::interactable::InteractionPrompt {
                text: prompt_text.to_string(),
                ..default()
            },
            crate::interactable::InteractableState::default(),
            Name::new(format!("Interactable_{:?}", interaction_type)),
        ))
        .id()
}

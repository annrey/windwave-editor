//! 可交互组件系统
//!
//! 提供游戏中常见的可交互物体组件和交互逻辑

use bevy::prelude::*;
use bevy::sprite::Sprite;
use log::info;

// ============================================================================
// 交互类型定义
// ============================================================================

/// 交互类型 - 定义物体如何被交互
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionType {
    Pickup,   // 拾取物品
    Dialogue, // 对话
    Trigger,  // 触发器
    Usable,   // 可使用物品
    Activate, // 激活开关
    Inspect,  // 检查/查看
}

/// 交互半径组件
#[derive(Component, Debug, Clone, Copy)]
pub struct InteractionRadius {
    pub radius: f32,
}

impl Default for InteractionRadius {
    fn default() -> Self {
        Self { radius: 50.0 }
    }
}

/// 交互提示组件 - 显示给玩家的提示
#[derive(Component, Debug, Clone)]
pub struct InteractionPrompt {
    pub text: String,
    pub key_hint: String,   // 例如 "E"
    pub show_distance: f32, // 多远距离开始显示
}

impl Default for InteractionPrompt {
    fn default() -> Self {
        Self {
            text: "交互".to_string(),
            key_hint: "E".to_string(),
            show_distance: 80.0,
        }
    }
}

/// 可交互标记组件
#[derive(Component, Default)]
pub struct Interactable;

/// 交互状态组件 - 跟踪物体的交互状态
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InteractableState {
    #[default]
    Idle,
    Hovered,     // 鼠标悬停
    Highlighted, // 玩家在附近
    Interacting, // 正在交互中
    Cooldown,    // 冷却中
    Disabled,    // 禁用
}

/// 交互冷却组件
#[derive(Component, Debug, Clone, Copy)]
pub struct InteractionCooldown {
    pub duration: f32,
    pub remaining: f32,
}

// ============================================================================
// 交互事件
// ============================================================================

/// 交互开始事件
#[derive(Message, Debug, Clone)]
pub struct InteractionStartEvent {
    pub player: Entity,
    pub target: Entity,
    pub interaction_type: InteractionType,
}

/// 交互完成事件
#[derive(Message, Debug, Clone)]
pub struct InteractionCompleteEvent {
    pub player: Entity,
    pub target: Entity,
    pub interaction_type: InteractionType,
    pub success: bool,
}

/// 交互取消事件
#[derive(Message, Debug, Clone)]
pub struct InteractionCancelEvent {
    pub player: Entity,
    pub target: Entity,
}

// ============================================================================
// 玩家交互状态机
// ============================================================================

/// 玩家交互状态
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayerInteractionState {
    #[default]
    Idle,
    LookingForInteractable,
    Hovering(Entity),
    Interacting(Entity),
}

/// 玩家交互配置
#[derive(Resource)]
pub struct PlayerInteractionConfig {
    pub max_interaction_distance: f32,
    pub interaction_duration: f32, // 交互需要的时间
}

impl Default for PlayerInteractionConfig {
    fn default() -> Self {
        Self {
            max_interaction_distance: 100.0,
            interaction_duration: 0.5,
        }
    }
}

/// 当前交互进度
#[derive(Resource, Default)]
pub struct InteractionProgress {
    pub target: Option<Entity>,
    pub progress: f32, // 0.0 - 1.0
}

// ============================================================================
// 可交互插件
// ============================================================================

pub struct InteractablePlugin;

impl Plugin for InteractablePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerInteractionState>()
            .init_resource::<PlayerInteractionConfig>()
            .init_resource::<InteractionProgress>()
            .add_message::<InteractionStartEvent>()
            .add_message::<InteractionCompleteEvent>()
            .add_message::<InteractionCancelEvent>()
            .add_systems(Update, detect_nearby_interactables)
            .add_systems(Update, update_interaction_highlight)
            .add_systems(Update, update_interaction_progress)
            .add_systems(Update, update_cooldowns);
    }
}

// ============================================================================
// 系统实现
// ============================================================================

/// 检测附近的可交互物体
fn detect_nearby_interactables(
    player_query: Query<&GlobalTransform, With<Camera>>, // 简化：用相机作为玩家
    interactable_query: Query<
        (
            Entity,
            &GlobalTransform,
            &InteractionRadius,
            Option<&InteractionPrompt>,
        ),
        With<Interactable>,
    >,
    config: Res<PlayerInteractionConfig>,
    mut player_state: ResMut<PlayerInteractionState>,
    mut interactable_states: Query<&mut InteractableState>,
) {
    // 获取玩家位置（用第一个相机简化）
    let player_pos = if let Ok(transform) = player_query.single() {
        transform.translation().truncate()
    } else {
        return;
    };

    // 找到最近的可交互物体
    let mut nearest: Option<(Entity, f32)> = None;

    for (entity, transform, radius, _prompt) in interactable_query.iter() {
        let target_pos = transform.translation().truncate();
        let distance = player_pos.distance(target_pos);

        if distance < config.max_interaction_distance {
            let actual_distance = distance - radius.radius;
            if actual_distance < nearest.map(|(_, d)| d).unwrap_or(f32::INFINITY) {
                nearest = Some((entity, actual_distance));
            }
        }
    }

    // 更新玩家状态
    match (*player_state, nearest) {
        (PlayerInteractionState::Idle, Some((entity, _))) => {
            *player_state = PlayerInteractionState::Hovering(entity);
            if let Ok(mut state) = interactable_states.get_mut(entity) {
                *state = InteractableState::Highlighted;
            }
        }
        (PlayerInteractionState::Hovering(current), Some((new_entity, _)))
            if current != new_entity =>
        {
            // 清除旧的高亮
            if let Ok(mut state) = interactable_states.get_mut(current) {
                *state = InteractableState::Idle;
            }
            // 设置新的
            *player_state = PlayerInteractionState::Hovering(new_entity);
            if let Ok(mut state) = interactable_states.get_mut(new_entity) {
                *state = InteractableState::Highlighted;
            }
        }
        (PlayerInteractionState::Hovering(current), None) => {
            // 没有物体在附近了
            if let Ok(mut state) = interactable_states.get_mut(current) {
                *state = InteractableState::Idle;
            }
            *player_state = PlayerInteractionState::Idle;
        }
        _ => {}
    }
}

/// 更新交互高亮视觉效果
fn update_interaction_highlight(mut query: Query<(&InteractableState, &mut Sprite)>) {
    for (state, mut sprite) in query.iter_mut() {
        match state {
            InteractableState::Idle => {
                // 恢复原色
            }
            InteractableState::Hovered | InteractableState::Highlighted => {
                // 高亮
                sprite.color = Color::srgb(1.2, 1.2, 1.2);
            }
            InteractableState::Interacting => {
                sprite.color = Color::srgb(0.8, 1.0, 0.8);
            }
            InteractableState::Cooldown => {
                sprite.color = Color::srgb(0.6, 0.6, 0.6);
            }
            InteractableState::Disabled => {
                sprite.color = Color::srgb(0.3, 0.3, 0.3);
            }
        }
    }
}

/// 更新交互进度（长按交互）
fn update_interaction_progress(
    time: Res<Time>,
    config: Res<PlayerInteractionConfig>,
    mut progress: ResMut<InteractionProgress>,
    player_state: Res<PlayerInteractionState>,
    mut complete_events: MessageWriter<InteractionCompleteEvent>,
    mut interactable_states: Query<&mut InteractableState>,
    interactable_types: Query<&InteractionType>,
) {
    if let PlayerInteractionState::Interacting(target) = *player_state {
        if progress.target == Some(target) {
            progress.progress += time.delta_secs() / config.interaction_duration;

            if progress.progress >= 1.0 {
                // 完成交互
                info!("交互完成！");

                if let Ok(interaction_type) = interactable_types.get(target) {
                    complete_events.write(InteractionCompleteEvent {
                        player: Entity::PLACEHOLDER, // 简化
                        target,
                        interaction_type: *interaction_type,
                        success: true,
                    });
                }

                if let Ok(mut state) = interactable_states.get_mut(target) {
                    *state = InteractableState::Idle;
                }

                progress.progress = 0.0;
                progress.target = None;
            }
        }
    }
}

/// 更新交互冷却
fn update_cooldowns(
    time: Res<Time>,
    mut query: Query<(&mut InteractionCooldown, &mut InteractableState)>,
) {
    for (mut cooldown, mut state) in query.iter_mut() {
        if cooldown.remaining > 0.0 {
            cooldown.remaining -= time.delta_secs();
            if cooldown.remaining <= 0.0 {
                cooldown.remaining = 0.0;
                *state = InteractableState::Idle;
            }
        }
    }
}

// ============================================================================
// 便捷函数
// ============================================================================

/// 开始交互
pub fn start_interaction(
    _commands: &mut Commands,
    player: Entity,
    target: Entity,
    interaction_type: InteractionType,
    progress: &mut ResMut<InteractionProgress>,
    player_state: &mut ResMut<PlayerInteractionState>,
    interactable_states: &mut Query<&mut InteractableState>,
    start_events: &mut MessageWriter<InteractionStartEvent>,
) {
    info!("开始交互");

    progress.target = Some(target);
    progress.progress = 0.0;
    **player_state = PlayerInteractionState::Interacting(target);

    if let Ok(mut state) = interactable_states.get_mut(target) {
        *state = InteractableState::Interacting;
    }

    start_events.write(InteractionStartEvent {
        player,
        target,
        interaction_type,
    });
}

/// 取消交互
pub fn cancel_interaction(
    target: Entity,
    progress: &mut ResMut<InteractionProgress>,
    player_state: &mut ResMut<PlayerInteractionState>,
    interactable_states: &mut Query<&mut InteractableState>,
    cancel_events: &mut MessageWriter<InteractionCancelEvent>,
    player: Entity,
) {
    info!("取消交互");

    progress.progress = 0.0;
    progress.target = None;
    **player_state = PlayerInteractionState::Idle;

    if let Ok(mut state) = interactable_states.get_mut(target) {
        *state = InteractableState::Idle;
    }

    cancel_events.write(InteractionCancelEvent { player, target });
}

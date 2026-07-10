//! 游戏输入系统
//!
//! 提供标准化的输入事件管理、输入映射和输入缓冲
//! 用于分离编辑器输入和游戏输入

use bevy::prelude::*;
use std::collections::HashMap;

// ============================================================================
// 输入动作定义
// ============================================================================

/// 游戏输入动作
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameInputAction {
    MoveForward,
    MoveBackward,
    MoveLeft,
    MoveRight,
    Jump,
    Interact,
    Attack,
    UseItem,
    OpenInventory,
    OpenMenu,
    Sprint,
    Crouch,
    PrimaryAction,
    SecondaryAction,
    TertiaryAction,
}

/// 输入阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputPhase {
    Pressed,  // 刚按下
    Held,     // 按住中
    Released, // 刚释放
}

/// 游戏输入事件
#[derive(Message, Debug, Clone)]
pub struct GameInputEvent {
    pub action: GameInputAction,
    pub phase: InputPhase,
    pub timestamp: f64,
    pub value: f32, // 用于模拟输入（如摇杆）
}

// ============================================================================
// 输入映射配置
// ============================================================================

/// 按键绑定
#[derive(Debug, Clone)]
pub enum KeyBinding {
    KeyCode(KeyCode),
    MouseButton(MouseButton),
    // 可以扩展游戏手柄等
}

/// 输入映射资源
#[derive(Resource, Clone)]
pub struct InputMapping {
    bindings: HashMap<GameInputAction, Vec<KeyBinding>>,
}

impl Default for InputMapping {
    fn default() -> Self {
        let mut bindings = HashMap::new();

        // 默认 WASD 移动
        bindings.insert(
            GameInputAction::MoveForward,
            vec![KeyBinding::KeyCode(KeyCode::KeyW)],
        );
        bindings.insert(
            GameInputAction::MoveBackward,
            vec![KeyBinding::KeyCode(KeyCode::KeyS)],
        );
        bindings.insert(
            GameInputAction::MoveLeft,
            vec![KeyBinding::KeyCode(KeyCode::KeyA)],
        );
        bindings.insert(
            GameInputAction::MoveRight,
            vec![KeyBinding::KeyCode(KeyCode::KeyD)],
        );

        // 跳跃
        bindings.insert(
            GameInputAction::Jump,
            vec![KeyBinding::KeyCode(KeyCode::Space)],
        );

        // 交互
        bindings.insert(
            GameInputAction::Interact,
            vec![KeyBinding::KeyCode(KeyCode::KeyE)],
        );

        // 攻击
        bindings.insert(
            GameInputAction::Attack,
            vec![KeyBinding::MouseButton(MouseButton::Left)],
        );

        // 背包
        bindings.insert(
            GameInputAction::OpenInventory,
            vec![KeyBinding::KeyCode(KeyCode::KeyI)],
        );

        // 菜单
        bindings.insert(
            GameInputAction::OpenMenu,
            vec![KeyBinding::KeyCode(KeyCode::Escape)],
        );

        // 冲刺
        bindings.insert(
            GameInputAction::Sprint,
            vec![KeyBinding::KeyCode(KeyCode::ShiftLeft)],
        );

        // 下蹲
        bindings.insert(
            GameInputAction::Crouch,
            vec![KeyBinding::KeyCode(KeyCode::ControlLeft)],
        );

        Self { bindings }
    }
}

impl InputMapping {
    /// 绑定动作到按键
    pub fn bind(&mut self, action: GameInputAction, binding: KeyBinding) {
        self.bindings.entry(action).or_default().push(binding);
    }

    /// 清除动作的所有绑定
    pub fn clear_bindings(&mut self, action: &GameInputAction) {
        self.bindings.remove(action);
    }

    /// 获取动作的绑定
    pub fn get_bindings(&self, action: &GameInputAction) -> &[KeyBinding] {
        self.bindings.get(action).map_or(&[], |v| v.as_slice())
    }

    /// 检查按键是否触发动作
    pub fn is_action_pressed(
        &self,
        action: &GameInputAction,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
    ) -> bool {
        if let Some(bindings) = self.bindings.get(action) {
            bindings.iter().any(|binding| match binding {
                KeyBinding::KeyCode(code) => keys.pressed(*code),
                KeyBinding::MouseButton(btn) => mouse.pressed(*btn),
            })
        } else {
            false
        }
    }

    /// 检查按键是否刚按下
    pub fn is_action_just_pressed(
        &self,
        action: &GameInputAction,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
    ) -> bool {
        if let Some(bindings) = self.bindings.get(action) {
            bindings.iter().any(|binding| match binding {
                KeyBinding::KeyCode(code) => keys.just_pressed(*code),
                KeyBinding::MouseButton(btn) => mouse.just_pressed(*btn),
            })
        } else {
            false
        }
    }

    /// 检查按键是否刚释放
    pub fn is_action_just_released(
        &self,
        action: &GameInputAction,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
    ) -> bool {
        if let Some(bindings) = self.bindings.get(action) {
            bindings.iter().any(|binding| match binding {
                KeyBinding::KeyCode(code) => keys.just_released(*code),
                KeyBinding::MouseButton(btn) => mouse.just_released(*btn),
            })
        } else {
            false
        }
    }
}

// ============================================================================
// 输入状态缓冲
// ============================================================================

/// 输入状态 - 记录上一帧的输入状态
#[derive(Resource, Default)]
pub struct InputStateBuffer {
    pressed_actions: Vec<GameInputAction>,
    last_frame_pressed: Vec<GameInputAction>,
    move_direction: Vec2,
}

impl InputStateBuffer {
    /// 获取移动方向（标准化向量）
    pub fn move_direction(&self) -> Vec2 {
        self.move_direction
    }

    /// 检查动作是否按住
    pub fn is_pressed(&self, action: &GameInputAction) -> bool {
        self.pressed_actions.contains(action)
    }

    /// 检查动作是否刚按下
    pub fn is_just_pressed(&self, action: &GameInputAction) -> bool {
        self.pressed_actions.contains(action) && !self.last_frame_pressed.contains(action)
    }

    /// 检查动作是否刚释放
    pub fn is_just_released(&self, action: &GameInputAction) -> bool {
        !self.pressed_actions.contains(action) && self.last_frame_pressed.contains(action)
    }
}

// ============================================================================
// 游戏输入插件
// ============================================================================

pub struct GameInputPlugin;

impl Plugin for GameInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputMapping>()
            .init_resource::<InputStateBuffer>()
            .add_message::<GameInputEvent>()
            .add_systems(PreUpdate, update_input_state)
            .add_systems(PreUpdate, emit_input_events);
    }
}

// ============================================================================
// 系统实现
// ============================================================================

/// 更新输入状态缓冲
fn update_input_state(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    input_mapping: Res<InputMapping>,
    mut buffer: ResMut<InputStateBuffer>,
) {
    // 保存上一帧状态
    buffer.last_frame_pressed = buffer.pressed_actions.clone();
    buffer.pressed_actions.clear();

    // 更新移动方向
    let mut dir = Vec2::ZERO;
    if input_mapping.is_action_pressed(&GameInputAction::MoveForward, &keys, &mouse) {
        dir.y += 1.0;
    }
    if input_mapping.is_action_pressed(&GameInputAction::MoveBackward, &keys, &mouse) {
        dir.y -= 1.0;
    }
    if input_mapping.is_action_pressed(&GameInputAction::MoveLeft, &keys, &mouse) {
        dir.x -= 1.0;
    }
    if input_mapping.is_action_pressed(&GameInputAction::MoveRight, &keys, &mouse) {
        dir.x += 1.0;
    }
    buffer.move_direction = dir.normalize_or_zero();

    // 更新所有动作状态
    let all_actions = [
        GameInputAction::MoveForward,
        GameInputAction::MoveBackward,
        GameInputAction::MoveLeft,
        GameInputAction::MoveRight,
        GameInputAction::Jump,
        GameInputAction::Interact,
        GameInputAction::Attack,
        GameInputAction::UseItem,
        GameInputAction::OpenInventory,
        GameInputAction::OpenMenu,
        GameInputAction::Sprint,
        GameInputAction::Crouch,
        GameInputAction::PrimaryAction,
        GameInputAction::SecondaryAction,
        GameInputAction::TertiaryAction,
    ];

    for action in all_actions {
        if input_mapping.is_action_pressed(&action, &keys, &mouse) {
            buffer.pressed_actions.push(action);
        }
    }
}

/// 发送输入事件
fn emit_input_events(
    buffer: Res<InputStateBuffer>,
    time: Res<Time>,
    mut events: MessageWriter<GameInputEvent>,
) {
    let timestamp = time.elapsed_secs_f64();

    // 检查刚按下的动作
    for action in &buffer.pressed_actions {
        if buffer.is_just_pressed(action) {
            events.write(GameInputEvent {
                action: action.clone(),
                phase: InputPhase::Pressed,
                timestamp,
                value: 1.0,
            });
        }
    }

    // 检查按住的动作（每帧发送）
    for action in &buffer.pressed_actions {
        if !buffer.is_just_pressed(action) {
            events.write(GameInputEvent {
                action: action.clone(),
                phase: InputPhase::Held,
                timestamp,
                value: 1.0,
            });
        }
    }

    // 检查刚释放的动作
    for action in &buffer.last_frame_pressed {
        if buffer.is_just_released(action) {
            events.write(GameInputEvent {
                action: action.clone(),
                phase: InputPhase::Released,
                timestamp,
                value: 0.0,
            });
        }
    }
}

// ============================================================================
// 便捷系统
// ============================================================================

/// 只有在播放模式下才启用游戏输入
pub fn game_input_active(play_state: Res<crate::play_mode::PlayModeState>) -> bool {
    play_state.is_playing
}

/// 条件运行集 - 仅在播放模式运行
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct GameInputActiveSet;

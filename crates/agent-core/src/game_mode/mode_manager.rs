//! Game Mode Manager - 游戏模式管理器
//!
//! 负责创建、加载、保存和管理游戏模式定义。

use super::mode_types::*;
use std::collections::HashMap;

/// 游戏模式管理器
#[derive(Debug, Clone)]
pub struct GameModeManager {
    modes: HashMap<String, GameModeDefinition>,
    active_mode_id: Option<String>,
}

impl GameModeManager {
    pub fn new() -> Self {
        let mut manager = Self {
            modes: HashMap::new(),
            active_mode_id: None,
        };
        
        // 注册预设模式
        manager.register_preset_modes();
        
        manager
    }

    /// 注册预设游戏模式
    fn register_preset_modes(&mut self) {
        // 文字冒险模式
        let text_adventure = GameModeDefinition::new(
            GameModeType::TextAdventure,
            "文字冒险"
        ).with_narrative_style(NarrativeStyle::EpicFantasy);
        self.register_mode(text_adventure);

        // AI对战模式
        let ai_battle = GameModeDefinition::new(
            GameModeType::AIBattle,
            "AI对战"
        ).with_narrative_style(NarrativeStyle::EpicFantasy);
        self.register_mode(ai_battle);

        // NPC沙盒模式
        let npc_sandbox = GameModeDefinition::new(
            GameModeType::NPCSandbox,
            "NPC沙盒"
        ).with_narrative_style(NarrativeStyle::ModernUrban);
        self.register_mode(npc_sandbox);

        // 角色扮演模式
        let chat_roleplay = GameModeDefinition::new(
            GameModeType::ChatRoleplay,
            "角色扮演"
        ).with_narrative_style(NarrativeStyle::Romance);
        self.register_mode(chat_roleplay);
    }

    /// 注册游戏模式
    pub fn register_mode(&mut self, mode: GameModeDefinition) {
        self.modes.insert(mode.id.clone(), mode);
    }

    /// 获取游戏模式
    pub fn get_mode(&self, id: &str) -> Option<&GameModeDefinition> {
        self.modes.get(id)
    }

    /// 获取可变的游戏模式
    pub fn get_mode_mut(&mut self, id: &str) -> Option<&mut GameModeDefinition> {
        self.modes.get_mut(id)
    }

    /// 获取所有模式
    pub fn all_modes(&self) -> Vec<&GameModeDefinition> {
        self.modes.values().collect()
    }

    /// 获取所有预设模式
    pub fn preset_modes(&self) -> Vec<&GameModeDefinition> {
        self.modes
            .values()
            .filter(|m| matches!(m.mode_type, 
                GameModeType::TextAdventure | 
                GameModeType::AIBattle | 
                GameModeType::NPCSandbox | 
                GameModeType::ChatRoleplay
            ))
            .collect()
    }

    /// 获取自定义模式
    pub fn custom_modes(&self) -> Vec<&GameModeDefinition> {
        self.modes
            .values()
            .filter(|m| matches!(m.mode_type, GameModeType::Custom(_)))
            .collect()
    }

    /// 按类型获取模式
    pub fn get_modes_by_type(&self, mode_type: &GameModeType) -> Vec<&GameModeDefinition> {
        self.modes
            .values()
            .filter(|m| m.mode_type == *mode_type)
            .collect()
    }

    /// 删除模式
    pub fn remove_mode(&mut self, id: &str) -> Option<GameModeDefinition> {
        self.modes.remove(id)
    }

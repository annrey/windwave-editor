
// WindWave Plugin System
// Inspired by Ruflo's plugin architecture
// ADR-0001: https://github.com/ruvnet/ruflo/blob/main/plugins/ruflo-swarm/README.md

use std::collections::HashMap;
use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PluginError {
    #[error("Plugin not found: {0}")]
    NotFound(String),
    
    #[error("Plugin already registered: {0}")]
    AlreadyRegistered(String),
    
    #[error("Plugin dependency not found: {0} requires {1}")]
    DependencyNotFound(String, String),
    
    #[error("Plugin initialization failed: {0}")]
    InitializationFailed(String, String),
    
    #[error("Plugin shutdown failed: {0}")]
    ShutdownFailed(String, String),
}

/// 插件上下文
pub struct PluginContext {
    /// 已加载的插件
    loaded_plugins: Vec<String>,
    /// 配置
    config: HashMap<String, String>,
}

impl PluginContext {
    pub fn new() -> Self {
        Self {
            loaded_plugins: Vec::new(),
            config: HashMap::new(),
        }
    }
    
    /// 检查插件是否已加载
    pub fn is_loaded(&self, name: &str) -> bool {
        self.loaded_plugins.contains(&name.to_string())
    }
    
    /// 标记插件为已加载
    pub fn mark_loaded(&mut self, name: &str) {
        self.loaded_plugins.push(name.to_string());
    }
    
    /// 获取配置
    pub fn get_config(&self, key: &str) -> Option<&String> {
        self.config.get(key)
    }
    
    /// 设置配置
    pub fn set_config(&mut self, key: &str, value: &str) {
        self.config.insert(key.to_string(), value.to_string());
    }
}

/// 插件 Trait
pub trait Plugin: Send + Sync + 'static {
    /// 插件名称
    fn name(&self) -> &'static str;
    
    /// 插件版本
    fn version(&self) -> &'static str {
        "0.1.0"
    }
    
    /// 插件描述
    fn description(&self) -> &'static str {
        ""
    }
    
    /// 依赖的其他插件
    fn dependencies(&self) -> Vec<&'static str> {
        Vec::new()
    }
    
    /// 初始化插件
    fn initialize(&mut self, ctx: &mut PluginContext) -> Result<(), PluginError>;
    
    /// 关闭插件
    fn shutdown(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}

/// 插件注册表
pub struct PluginRegistry {
    plugins: HashMap<String, Box<dyn Plugin>>,
    load_order: Vec<String>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            load_order: Vec::new(),
        }
    }
    
    /// 注册插件
    pub fn register<P: Plugin + 'static>(&mut self, plugin: P) -> Result<(), PluginError> {
        let name = plugin.name().to_string();
        
        if self.plugins.contains_key(&name) {
            return Err(PluginError::AlreadyRegistered(name));
        }
        
        self.plugins.insert(name, Box::new(plugin));
        Ok(())
    }
    
    /// 初始化所有插件
    pub fn initialize_all(&mut self) -> Result<(), PluginError> {
        let mut ctx = PluginContext::new();
        let mut visited = std::collections::HashSet::new();
        
        // 按注册顺序初始化，但先处理依赖
        let plugin_names: Vec<_> = self.plugins.keys().cloned().collect();
        for name in &plugin_names {
            self.initialize_one(name, &mut ctx, &mut visited)?;
        }
        
        Ok(())
    }
    
    /// 初始化单个插件
    fn initialize_one(
        &mut self,
        name: &str,
        ctx: &mut PluginContext,
        visited: &mut std::collections::HashSet<String>,
    ) -> Result<(), PluginError> {
        if visited.contains(name) {
            return Ok(());
        }
        
        if ctx.is_loaded(name) {
            return Ok(());
        }
        
        visited.insert(name.to_string());
        
        let plugin = self
            .plugins
            .get_mut(name)
            .ok_or_else(|| PluginError::NotFound(name.to_string()))?;
        
        // 先初始化依赖
        for dep in plugin.dependencies() {
            if !ctx.is_loaded(dep) {
                if !self.plugins.contains_key(dep) {
                    return Err(PluginError::DependencyNotFound(
                        name.to_string(),
                        dep.to_string(),
                    ));
                }
                self.initialize_one(dep, ctx, visited)?;
            }
        }
        
        // 初始化插件
        plugin.initialize(ctx)?;
        ctx.mark_loaded(name);
        self.load_order.push(name.to_string());
        
        Ok(())
    }
    
    /// 关闭所有插件（按相反顺序）
    pub fn shutdown_all(&mut self) -> Result<(), PluginError> {
        for name in self.load_order.iter().rev() {
            if let Some(plugin) = self.plugins.get_mut(name) {
                plugin.shutdown()?;
            }
        }
        Ok(())
    }
    
    /// 获取已加载的插件列表
    pub fn loaded_plugins(&self) -> &[String] {
        &self.load_order
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // 测试插件 1
    struct TestPlugin1;
    
    impl Plugin for TestPlugin1 {
        fn name(&self) -> &'static str {
            "test-plugin-1"
        }
        
        fn initialize(&mut self, _ctx: &mut PluginContext) -> Result<(), PluginError> {
            Ok(())
        }
    }
    
    // 测试插件 2（依赖插件 1）
    struct TestPlugin2;
    
    impl Plugin for TestPlugin2 {
        fn name(&self) -> &'static str {
            "test-plugin-2"
        }
        
        fn dependencies(&self) -> Vec<&'static str> {
            vec!["test-plugin-1"]
        }
        
        fn initialize(&mut self, _ctx: &mut PluginContext) -> Result<(), PluginError> {
            Ok(())
        }
    }
    
    #[test]
    fn test_plugin_registration() {
        let mut registry = PluginRegistry::new();
        registry.register(TestPlugin1).unwrap();
        registry.register(TestPlugin2).unwrap();
        
        assert!(matches!(
            registry.register(TestPlugin1),
            Err(PluginError::AlreadyRegistered(_))
        ));
    }
    
    #[test]
    fn test_plugin_initialization() {
        let mut registry = PluginRegistry::new();
        registry.register(TestPlugin1).unwrap();
        registry.register(TestPlugin2).unwrap();
        
        registry.initialize_all().unwrap();
        
        assert_eq!(registry.loaded_plugins(), &vec!["test-plugin-1", "test-plugin-2"]);
    }
}

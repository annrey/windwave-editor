
// WindWave Layered Controller Registry
// Inspired by Ruflo's controller-registry.ts
// ADR-0003: https://github.com/ruvnet/ruflo/blob/main/v3/@claude-flow/memory/src/controller-registry.ts

use std::collections::{HashMap, HashSet};
use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ControllerError {
    #[error("Controller not found: {0}")]
    NotFound(String),
    
    #[error("Controller already registered: {0}")]
    AlreadyRegistered(String),
    
    #[error("Dependency not satisfied: {0} requires {1}")]
    UnsatisfiedDependency(String, String),
    
    #[error("Initialization failed for {0}: {1}")]
    InitializationFailed(String, String),
    
    #[error("Shutdown failed for {0}: {1}")]
    ShutdownFailed(String, String),
    
    #[error("Circular dependency detected: {0}")]
    CircularDependency(String),
}

/// 初始化层级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum InitLevel {
    /// 基础层（预存在）
    Foundation = 0,
    /// 核心智能层
    Core = 1,
    /// 图谱 + 安全层
    Graph = 2,
    /// 专业化层
    Specialization = 3,
    /// 因果 + 路由层
    Causal = 4,
    /// 高级服务层
    Advanced = 5,
    /// 会话管理层
    Session = 6,
}

impl InitLevel {
    /// 迭代所有层级（按顺序）
    pub fn iter() -> impl Iterator<Item = Self> {
        use InitLevel::*;
        [
            Foundation,
            Core,
            Graph,
            Specialization,
            Causal,
            Advanced,
            Session,
        ]
        .iter()
        .copied()
    }
}

impl fmt::Display for InitLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InitLevel::Foundation => write!(f, "Foundation"),
            InitLevel::Core => write!(f, "Core"),
            InitLevel::Graph => write!(f, "Graph"),
            InitLevel::Specialization => write!(f, "Specialization"),
            InitLevel::Causal => write!(f, "Causal"),
            InitLevel::Advanced => write!(f, "Advanced"),
            InitLevel::Session => write!(f, "Session"),
        }
    }
}

/// 控制器上下文 - 初始化时传递给控制器
pub struct ControllerContext {
    /// 已初始化的控制器
    initialized: HashSet<String>,
    /// 全局配置
    config: HashMap<String, String>,
}

impl ControllerContext {
    pub fn new() -> Self {
        Self {
            initialized: HashSet::new(),
            config: HashMap::new(),
        }
    }
    
    /// 检查依赖是否已初始化
    pub fn is_initialized(&self, name: &str) -> bool {
        self.initialized.contains(name)
    }
    
    /// 标记控制器为已初始化
    pub fn mark_initialized(&mut self, name: &str) {
        self.initialized.insert(name.to_string());
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

/// 控制器 Trait
pub trait Controller: Send + Sync + 'static {
    /// 控制器名称
    fn name(&self) -> &'static str;
    
    /// 初始化层级
    fn init_level(&self) -> InitLevel;
    
    /// 依赖的其他控制器
    fn dependencies(&self) -> Vec<&'static str> {
        Vec::new()
    }
    
    /// 初始化控制器
    fn initialize(&mut self, ctx: &mut ControllerContext) -> Result<(), ControllerError>;
    
    /// 关闭控制器
    fn shutdown(&mut self) -> Result<(), ControllerError> {
        Ok(())
    }
}

/// 控制器注册表
pub struct ControllerRegistry {
    controllers: HashMap<String, Box<dyn Controller>>,
    initialization_order: Vec<String>,
}

impl ControllerRegistry {
    pub fn new() -> Self {
        Self {
            controllers: HashMap::new(),
            initialization_order: Vec::new(),
        }
    }
    
    /// 注册控制器
    pub fn register<C: Controller + 'static>(&mut self, controller: C) -> Result<(), ControllerError> {
        let name = controller.name().to_string();
        
        if self.controllers.contains_key(&name) {
            return Err(ControllerError::AlreadyRegistered(name));
        }
        
        self.controllers.insert(name, Box::new(controller));
        Ok(())
    }
    
    /// 获取某一层级的所有控制器（按注册顺序）
    fn controllers_by_level(&self, level: InitLevel) -> Vec<&String> {
        self.controllers
            .iter()
            .filter(|(_, c)| c.init_level() == level)
            .map(|(name, _)| name)
            .collect()
    }
    
    /// 初始化所有控制器（按层级顺序）
    pub fn initialize_all(&mut self) -> Result<(), ControllerError> {
        let mut ctx = ControllerContext::new();
        let mut visited = HashSet::new();
        
        for level in InitLevel::iter() {
            for name in self.controllers_by_level(level) {
                self.initialize_one(name, &mut ctx, &mut visited)?;
            }
        }
        
        Ok(())
    }
    
    /// 初始化单个控制器
    fn initialize_one(
        &mut self,
        name: &str,
        ctx: &mut ControllerContext,
        visited: &mut HashSet<String>,
    ) -> Result<(), ControllerError> {
        if visited.contains(name) {
            return Ok(());
        }
        
        // 防止循环依赖
        if ctx.is_initialized(name) {
            return Ok(());
        }
        
        visited.insert(name.to_string());
        
        let controller = self
            .controllers
            .get_mut(name)
            .ok_or_else(|| ControllerError::NotFound(name.to_string()))?;
        
        // 先初始化依赖
        for dep in controller.dependencies() {
            if !ctx.is_initialized(dep) {
                self.initialize_one(dep, ctx, visited)?;
            }
        }
        
        // 验证所有依赖已初始化
        for dep in controller.dependencies() {
            if !ctx.is_initialized(dep) {
                return Err(ControllerError::UnsatisfiedDependency(
                    name.to_string(),
                    dep.to_string(),
                ));
            }
        }
        
        // 初始化控制器
        controller.initialize(ctx)?;
        ctx.mark_initialized(name);
        self.initialization_order.push(name.to_string());
        
        Ok(())
    }
    
    /// 关闭所有控制器（按相反顺序）
    pub fn shutdown_all(&mut self) -> Result<(), ControllerError> {
        for name in self.initialization_order.iter().rev() {
            if let Some(controller) = self.controllers.get_mut(name) {
                controller.shutdown()?;
            }
        }
        Ok(())
    }
    
    /// 获取已初始化的控制器列表
    pub fn initialized(&self) -> &[String] {
        &self.initialization_order
    }
}

impl Default for ControllerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // 测试用的控制器 1
    struct TestController1;
    
    impl Controller for TestController1 {
        fn name(&self) -> &'static str {
            "test-controller-1"
        }
        
        fn init_level(&self) -> InitLevel {
            InitLevel::Core
        }
        
        fn initialize(&mut self, _ctx: &mut ControllerContext) -> Result<(), ControllerError> {
            Ok(())
        }
    }
    
    // 测试用的控制器 2（依赖控制器 1）
    struct TestController2;
    
    impl Controller for TestController2 {
        fn name(&self) -> &'static str {
            "test-controller-2"
        }
        
        fn init_level(&self) -> InitLevel {
            InitLevel::Specialization
        }
        
        fn dependencies(&self) -> Vec<&'static str> {
            vec!["test-controller-1"]
        }
        
        fn initialize(&mut self, _ctx: &mut ControllerContext) -> Result<(), ControllerError> {
            Ok(())
        }
    }
    
    #[test]
    fn test_controller_registration() {
        let mut registry = ControllerRegistry::new();
        registry.register(TestController1).unwrap();
        registry.register(TestController2).unwrap();
        
        // 重复注册应该失败
        assert!(matches!(
            registry.register(TestController1),
            Err(ControllerError::AlreadyRegistered(_))
        ));
    }
    
    #[test]
    fn test_initialization_order() {
        let mut registry = ControllerRegistry::new();
        registry.register(TestController1).unwrap();
        registry.register(TestController2).unwrap();
        
        registry.initialize_all().unwrap();
        
        assert_eq!(registry.initialized(), &vec!["test-controller-1", "test-controller-2"]);
    }
}

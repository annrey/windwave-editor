
// WindWave Namespace System
// Inspired by Ruflo's ruflo-agentdb namespace convention
// ADR-0002: https://github.com/ruvnet/ruflo/blob/main/plugins/ruflo-agentdb/README.md#namespace-convention

use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum NamespaceError {
    #[error("Invalid character in namespace: {0}")]
    InvalidCharacter(String),
    
    #[error("Namespace too long (max 200 chars): {0}")]
    TooLong(String),
    
    #[error("Namespace cannot be empty")]
    Empty,
    
    #[error("Namespace cannot contain colon: {0}")]
    ContainsColon(String),
    
    #[error("Reserved namespace: {0}")]
    Reserved(String),
}

/// Namespace - 命名空间
/// 
/// Format: `<plugin-stem>-<intent>` (kebab-case)
/// 
/// # Reserved Namespaces (cannot be used by plugins)
/// - `pattern` - Pattern learning (ReasoningBank)
/// - `claude-memories` - Claude Code bridge
/// - `default` - Default storage
/// - `system` - System configuration
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Namespace(String);

impl Namespace {
    /// 验证并创建新的命名空间
    pub fn new(s: &str) -> Result<Self, NamespaceError> {
        Self::validate(s)?;
        Ok(Self(s.to_string()))
    }
    
    /// 创建命名空间，不验证（仅用于系统内部）
    pub(crate) fn new_unchecked(s: &str) -> Self {
        Self(s.to_string())
    }
    
    /// 验证命名空间格式
    pub fn validate(s: &str) -> Result<(), NamespaceError> {
        if s.is_empty() {
            return Err(NamespaceError::Empty);
        }
        
        if s.len() > 200 {
            return Err(NamespaceError::TooLong(s.to_string()));
        }
        
        if s.contains(':') {
            return Err(NamespaceError::ContainsColon(s.to_string()));
        }
        
        // 检查每个字符
        for c in s.chars() {
            if !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '-' {
                return Err(NamespaceError::InvalidCharacter(s.to_string()));
            }
        }
        
        // 检查保留命名空间
        if Self::is_reserved(s) {
            return Err(NamespaceError::Reserved(s.to_string()));
        }
        
        Ok(())
    }
    
    /// 检查是否是保留命名空间
    pub fn is_reserved(s: &str) -> bool {
        matches!(s, "pattern" | "claude-memories" | "default" | "system")
    }
    
    /// 获取保留的命名空间列表
    pub fn reserved() -> Vec<&'static str> {
        vec!["pattern", "claude-memories", "default", "system"]
    }
    
    /// 获取项目特定的命名空间列表
    pub fn project_specific() -> Vec<&'static str> {
        vec![
            "scene-entities",
            "scene-components", 
            "skill-templates",
            "tool-registry",
            "user-preferences",
            "edit-history",
            "agent-state",
        ]
    }
    
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Namespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<&str> for Namespace {
    type Error = NamespaceError;
    
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl TryFrom<String> for Namespace {
    type Error = NamespaceError;
    
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(&s)
    }
}

/// 构建命名空间的辅助函数
pub mod builder {
    use super::*;
    
    /// 创建场景相关命名空间
    pub fn scene(intent: &str) -> Result<Namespace, NamespaceError> {
        Namespace::new(&format!("scene-{}", intent))
    }
    
    /// 创建技能相关命名空间
    pub fn skill(intent: &str) -> Result<Namespace, NamespaceError> {
        Namespace::new(&format!("skill-{}", intent))
    }
    
    /// 创建工具相关命名空间
    pub fn tool(intent: &str) -> Result<Namespace, NamespaceError> {
        Namespace::new(&format!("tool-{}", intent))
    }
    
    /// 创建代理相关命名空间
    pub fn agent(intent: &str) -> Result<Namespace, NamespaceError> {
        Namespace::new(&format!("agent-{}", intent))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_valid_namespace() {
        assert!(Namespace::new("scene-entities").is_ok());
        assert!(Namespace::new("skill-templates").is_ok());
        assert!(Namespace::new("my-plugin-my-intent").is_ok());
    }
    
    #[test]
    fn test_invalid_namespace() {
        assert!(matches!(Namespace::new(""), Err(NamespaceError::Empty)));
        assert!(matches!(Namespace::new("Invalid"), Err(NamespaceError::InvalidCharacter(_))));
        assert!(matches!(Namespace::new("with:colon"), Err(NamespaceError::ContainsColon(_))));
    }
    
    #[test]
    fn test_reserved_namespace() {
        assert!(Namespace::is_reserved("pattern"));
        assert!(Namespace::is_reserved("default"));
        assert!(matches!(Namespace::new("pattern"), Err(NamespaceError::Reserved(_))));
    }
    
    #[test]
    fn test_namespace_builder() {
        assert_eq!(builder::scene("entities").unwrap().as_str(), "scene-entities");
        assert_eq!(builder::skill("templates").unwrap().as_str(), "skill-templates");
    }
}

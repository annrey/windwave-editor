//! Goal Checker - 目标状态检测器
//!
//! GoalChecker 用于验证编辑操作是否达到了预期的目标状态。
//! 通过定义 GoalRequirementKind 来描述期望的场景状态，
//! 然后与实际的 SceneEntityInfo 列表进行对比检查。
//!
//! # 支持的目标类型
//! - EntityExists: 检查实体是否存在
//! - HasComponent: 检查实体是否拥有特定组件
//! - TransformNear: 检查实体位置是否在容差范围内
//! - SpriteColorIs: 检查实体精灵颜色是否匹配
//!
//! # 示例
//! ```ignore
//! let checker = GoalChecker::new();
//! let requirements = vec![
//!     GoalRequirementKind::EntityExists { name: "Enemy".into() },
//!     GoalRequirementKind::SpriteColorIs {
//!         entity_name: "Enemy".into(),
//!         rgba: [1.0, 0.0, 0.0, 1.0],
//!     },
//! ];
//! let scene = vec![SceneEntityInfo {
//!     name: "Enemy".into(),
//!     components: vec!["Sprite".into()],
//!     translation: Some([0.0, 0.0, 0.0]),
//!     sprite_color: Some([1.0, 0.0, 0.0, 1.0]),
//! }];
//! let result = checker.check(&requirements, &scene);
//! assert!(result.all_matched);
//! ```

use crate::goal::{GoalCheckResult, GoalRequirementKind, GoalRequirementResult};

/// 场景实体的运行时快照信息
///
/// 包含实体的名称、组件列表、位置和精灵颜色等属性，
/// 用于 GoalChecker 进行目标状态比对。
#[derive(Debug, Clone)]
pub struct SceneEntityInfo {
    /// 实体名称
    pub name: String,
    /// 实体挂载的组件类型列表（如 "Transform", "Sprite", "RigidBody"）
    pub components: Vec<String>,
    /// Transform 组件的 translation 值 [x, y, z]
    pub translation: Option<[f32; 3]>,
    /// Sprite 组件的颜色值 [r, g, b, a]
    pub sprite_color: Option<[f32; 4]>,
}

// ============================================================================
// GoalChecker 实现
// ============================================================================

/// 目标状态检测器
///
/// GoalChecker 根据预设的目标需求（GoalRequirementKind），
/// 逐一检查当前场景状态（SceneEntityInfo 列表）是否满足要求。
///
/// 适用于编辑操作的验证阶段，确保操作结果符合用户意图。
pub struct GoalChecker;

impl GoalChecker {
    /// 创建一个新的 GoalChecker 实例
    pub fn new() -> Self {
        Self
    }

    /// 检查所有目标需求是否被当前场景状态满足
    ///
    /// # 参数
    /// - `requirements`: 目标需求的列表
    /// - `scene`: 当前场景的实体信息列表
    ///
    /// # 返回
    /// GoalCheckResult，包含整体的成功/失败状态和每项需求的详细结果。
    pub fn check(
        &self,
        requirements: &[GoalRequirementKind],
        scene: &[SceneEntityInfo],
    ) -> GoalCheckResult {
        let results: Vec<GoalRequirementResult> = requirements
            .iter()
            .enumerate()
            .map(|(i, req)| {
                let matched = self.check_requirement(req, scene);
                GoalRequirementResult {
                    requirement_id: format!("req_{}", i),
                    matched,
                    description: self.describe_requirement(req),
                    message: if matched {
                        None
                    } else {
                        Some(format!("需求未满足: {}", self.describe_requirement(req)))
                    },
                }
            })
            .collect();

        GoalCheckResult {
            all_matched: results.iter().all(|r| r.matched),
            requirement_results: results,
        }
    }

    /// 检查单个目标需求是否满足
    fn check_requirement(
        &self,
        req: &GoalRequirementKind,
        scene: &[SceneEntityInfo],
    ) -> bool {
        match req {
            GoalRequirementKind::EntityExists { name } => {
                scene.iter().any(|e| e.name == *name)
            }

            GoalRequirementKind::HasComponent {
                entity_name,
                component,
            } => {
                scene.iter().any(|e| {
                    e.name == *entity_name
                        && e.components
                            .iter()
                            .any(|c| c.to_lowercase().contains(&component.to_lowercase()))
                })
            }

            GoalRequirementKind::TransformNear {
                entity_name,
                translation,
                tolerance,
            } => {
                scene.iter().any(|e| {
                    if e.name == *entity_name {
                        if let Some(t) = e.translation {
                            let dist = ((t[0] - translation[0]).powi(2)
                                + (t[1] - translation[1]).powi(2)
                                + (t[2] - translation[2]).powi(2))
                            .sqrt();
                            return dist <= *tolerance;
                        }
                    }
                    false
                })
            }

            GoalRequirementKind::SpriteColorIs {
                entity_name,
                rgba,
            } => {
                scene.iter().any(|e| {
                    if e.name == *entity_name {
                        if let Some(c) = e.sprite_color {
                            return (c[0] - rgba[0]).abs() < 0.01
                                && (c[1] - rgba[1]).abs() < 0.01
                                && (c[2] - rgba[2]).abs() < 0.01
                                && (c[3] - rgba[3]).abs() < 0.01;
                        }
                    }
                    false
                })
            }
        }
    }

    /// 生成目标需求的可读描述文本
    fn describe_requirement(&self, req: &GoalRequirementKind) -> String {
        match req {
            GoalRequirementKind::EntityExists { name } => {
                format!("场景中存在实体 \"{}\"", name)
            }
            GoalRequirementKind::HasComponent {
                entity_name,
                component,
            } => {
                format!("实体 \"{}\" 拥有组件 \"{}\"", entity_name, component)
            }
            GoalRequirementKind::TransformNear {
                entity_name,
                translation,
                tolerance,
            } => {
                format!(
                    "实体 \"{}\" 的位置在 [{:.2}, {:.2}, {:.2}] 的 {:.2} 单位范围内",
                    entity_name, translation[0], translation[1], translation[2], tolerance
                )
            }
            GoalRequirementKind::SpriteColorIs {
                entity_name,
                rgba,
            } => {
                format!(
                    "实体 \"{}\" 的 Sprite 颜色为 RGBA({:.2}, {:.2}, {:.2}, {:.2})",
                    entity_name, rgba[0], rgba[1], rgba[2], rgba[3]
                )
            }
        }
    }
}

impl Default for GoalChecker {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建一个测试用的 Enemy 实体信息
    fn enemy_entity() -> SceneEntityInfo {
        SceneEntityInfo {
            name: "Enemy".to_string(),
            components: vec![
                "Transform".to_string(),
                "Sprite".to_string(),
                "EnemyAI".to_string(),
            ],
            translation: Some([3.0, 0.0, 0.0]),
            sprite_color: Some([1.0, 0.0, 0.0, 1.0]), // 红色
        }
    }

    /// 创建一个测试用的 Player 实体信息
    fn player_entity() -> SceneEntityInfo {
        SceneEntityInfo {
            name: "Player".to_string(),
            components: vec!["Transform".to_string(), "PlayerController".to_string()],
            translation: Some([0.0, 0.0, 0.0]),
            sprite_color: Some([0.0, 0.0, 1.0, 1.0]), // 蓝色
        }
    }

    /// 创建一个标准的测试场景
    fn test_scene() -> Vec<SceneEntityInfo> {
        vec![enemy_entity(), player_entity()]
    }

    #[test]
    fn test_new_checker() {
        let checker = GoalChecker::new();
        let _ = checker;
    }

    #[test]
    fn test_default_checker() {
        let checker = GoalChecker::default();
        let _ = checker;
    }

    #[test]
    fn test_check_entity_exists_positive() {
        let checker = GoalChecker::new();
        let requirements = vec![GoalRequirementKind::EntityExists {
            name: "Enemy".to_string(),
        }];
        let result = checker.check(&requirements, &test_scene());
        assert!(result.all_matched);
    }

    #[test]
    fn test_check_entity_exists_negative() {
        let checker = GoalChecker::new();
        let requirements = vec![GoalRequirementKind::EntityExists {
            name: "NPC".to_string(),
        }];
        let result = checker.check(&requirements, &test_scene());
        assert!(!result.all_matched);
    }

    #[test]
    fn test_check_has_component_positive() {
        let checker = GoalChecker::new();
        let requirements = vec![GoalRequirementKind::HasComponent {
            entity_name: "Enemy".to_string(),
            component: "Sprite".to_string(),
        }];
        let result = checker.check(&requirements, &test_scene());
        assert!(result.all_matched);
    }

    #[test]
    fn test_check_has_component_negative() {
        let checker = GoalChecker::new();
        let requirements = vec![GoalRequirementKind::HasComponent {
            entity_name: "Enemy".to_string(),
            component: "RigidBody".to_string(),
        }];
        let result = checker.check(&requirements, &test_scene());
        assert!(!result.all_matched);
    }

    #[test]
    fn test_check_transform_near_positive() {
        let checker = GoalChecker::new();
        let requirements = vec![GoalRequirementKind::TransformNear {
            entity_name: "Enemy".to_string(),
            translation: [3.0, 0.0, 0.0],
            tolerance: 0.1,
        }];
        let result = checker.check(&requirements, &test_scene());
        assert!(result.all_matched);
    }

    #[test]
    fn test_check_transform_near_with_tolerance() {
        let checker = GoalChecker::new();
        // Enemy 在 [3, 0, 0]，目标 [3.5, 0, 0]，距离 0.5，容差 1.0 -> 应该满足
        let requirements = vec![GoalRequirementKind::TransformNear {
            entity_name: "Enemy".to_string(),
            translation: [3.5, 0.0, 0.0],
            tolerance: 1.0,
        }];
        let result = checker.check(&requirements, &test_scene());
        assert!(result.all_matched);
    }

    #[test]
    fn test_check_transform_near_negative() {
        let checker = GoalChecker::new();
        // Enemy 在 [3, 0, 0]，目标 [10, 0, 0]，距离 7，容差 0.1 -> 不满足
        let requirements = vec![GoalRequirementKind::TransformNear {
            entity_name: "Enemy".to_string(),
            translation: [10.0, 0.0, 0.0],
            tolerance: 0.1,
        }];
        let result = checker.check(&requirements, &test_scene());
        assert!(!result.all_matched);
    }

    #[test]
    fn test_check_sprite_color_positive() {
        let checker = GoalChecker::new();
        let requirements = vec![GoalRequirementKind::SpriteColorIs {
            entity_name: "Enemy".to_string(),
            rgba: [1.0, 0.0, 0.0, 1.0], // 红色
        }];
        let result = checker.check(&requirements, &test_scene());
        assert!(result.all_matched);
    }

    #[test]
    fn test_check_sprite_color_negative() {
        let checker = GoalChecker::new();
        let requirements = vec![GoalRequirementKind::SpriteColorIs {
            entity_name: "Enemy".to_string(),
            rgba: [0.0, 1.0, 0.0, 1.0], // 期望绿色，实际是红色
        }];
        let result = checker.check(&requirements, &test_scene());
        assert!(!result.all_matched);
    }

    #[test]
    fn test_check_multiple_requirements_all_pass() {
        let checker = GoalChecker::new();
        let requirements = vec![
            GoalRequirementKind::EntityExists {
                name: "Enemy".to_string(),
            },
            GoalRequirementKind::HasComponent {
                entity_name: "Enemy".to_string(),
                component: "Sprite".to_string(),
            },
            GoalRequirementKind::SpriteColorIs {
                entity_name: "Enemy".to_string(),
                rgba: [1.0, 0.0, 0.0, 1.0],
            },
        ];
        let result = checker.check(&requirements, &test_scene());
        assert!(result.all_matched);
        assert_eq!(result.requirement_results.len(), 3);
        // 每项都应该 matched
        for req_result in &result.requirement_results {
            assert!(req_result.matched);
        }
    }

    #[test]
    fn test_check_multiple_requirements_partial_fail() {
        let checker = GoalChecker::new();
        let requirements = vec![
            GoalRequirementKind::EntityExists {
                name: "Enemy".to_string(),
            }, // pass
            GoalRequirementKind::EntityExists {
                name: "NPC".to_string(),
            }, // fail
        ];
        let result = checker.check(&requirements, &test_scene());
        assert!(!result.all_matched);

        // 第一项应该成功
        assert!(result.requirement_results[0].matched);
        // 第二项应该失败
        assert!(!result.requirement_results[1].matched);
        // 失败的项应该有 message
        assert!(result.requirement_results[1].message.is_some());
    }

    #[test]
    fn test_check_empty_requirements() {
        let checker = GoalChecker::new();
        let requirements: Vec<GoalRequirementKind> = vec![];
        let result = checker.check(&requirements, &test_scene());
        // 没有需求时，all_matched 应为 true（空集合全匹配）
        assert!(result.all_matched);
        assert!(result.requirement_results.is_empty());
    }

    #[test]
    fn test_check_empty_scene() {
        let checker = GoalChecker::new();
        let requirements = vec![GoalRequirementKind::EntityExists {
            name: "Enemy".to_string(),
        }];
        let empty_scene: Vec<SceneEntityInfo> = vec![];
        let result = checker.check(&requirements, &empty_scene);
        assert!(!result.all_matched);
    }

    #[test]
    fn test_result_requirement_id_format() {
        let checker = GoalChecker::new();
        let requirements = vec![
            GoalRequirementKind::EntityExists {
                name: "A".to_string(),
            },
            GoalRequirementKind::EntityExists {
                name: "B".to_string(),
            },
            GoalRequirementKind::EntityExists {
                name: "C".to_string(),
            },
        ];
        let result = checker.check(&requirements, &test_scene());
        assert_eq!(result.requirement_results[0].requirement_id, "req_0");
        assert_eq!(result.requirement_results[1].requirement_id, "req_1");
        assert_eq!(result.requirement_results[2].requirement_id, "req_2");
    }

    #[test]
    fn test_check_has_component_case_insensitive() {
        let checker = GoalChecker::new();
        // Enemy 有 "Sprite" 组件，要求 "sprite"（小写）也应该匹配
        let requirements = vec![GoalRequirementKind::HasComponent {
            entity_name: "Enemy".to_string(),
            component: "sprite".to_string(),
        }];
        let result = checker.check(&requirements, &test_scene());
        assert!(result.all_matched);
    }

    #[test]
    fn test_describe_requirement_entity_exists() {
        let checker = GoalChecker::new();
        let req = GoalRequirementKind::EntityExists {
            name: "Enemy".to_string(),
        };
        let desc = checker.describe_requirement(&req);
        assert!(desc.contains("Enemy"));
        assert!(desc.contains("存在"));
    }

    // ---- 实体无 Sprite 颜色时检查 SpriteColorIs ——

    #[test]
    fn test_check_sprite_color_returns_false_when_no_color() {
        let checker = GoalChecker::new();
        let entity_without_color = SceneEntityInfo {
            name: "Ghost".to_string(),
            components: vec!["Transform".to_string()],
            translation: Some([0.0, 0.0, 0.0]),
            sprite_color: None, // 没有精灵颜色
        };
        let scene = vec![entity_without_color];
        let requirements = vec![GoalRequirementKind::SpriteColorIs {
            entity_name: "Ghost".to_string(),
            rgba: [1.0, 1.0, 1.0, 1.0],
        }];
        let result = checker.check(&requirements, &scene);
        assert!(!result.all_matched);
    }

    // ---- 实体无 Translation 时检查 TransformNear ——

    #[test]
    fn test_check_transform_near_returns_false_when_no_translation() {
        let checker = GoalChecker::new();
        let entity_without_transform = SceneEntityInfo {
            name: "Static".to_string(),
            components: vec!["Sprite".to_string()],
            translation: None, // 没有位置信息
            sprite_color: None,
        };
        let scene = vec![entity_without_transform];
        let requirements = vec![GoalRequirementKind::TransformNear {
            entity_name: "Static".to_string(),
            translation: [0.0, 0.0, 0.0],
            tolerance: 1.0,
        }];
        let result = checker.check(&requirements, &scene);
        assert!(!result.all_matched);
    }
}

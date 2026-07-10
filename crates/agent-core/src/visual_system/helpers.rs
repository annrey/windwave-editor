use super::*;

/// 颜色近似比较（允许小误差）
pub fn color_approx_eq(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() < 0.1)
}

/// 构建场景快照的文本摘要（用于 vision 分析）
pub fn build_scene_summary(snapshot: &[crate::goal_checker::SceneEntityInfo]) -> String {
    let mut parts = Vec::new();
    parts.push(format!("Total entities: {}", snapshot.len()));
    for info in snapshot {
        let mut desc = format!("- {} (components: {:?})", info.name, info.components);
        if let Some(translation) = info.translation {
            desc.push_str(&format!(
                " pos=[{:.1}, {:.1}, {:.1}]",
                translation[0], translation[1], translation[2]
            ));
        }
        if let Some(rgba) = info.sprite_color {
            desc.push_str(&format!(
                " color=[{:.2}, {:.2}, {:.2}, {:.2}]",
                rgba[0], rgba[1], rgba[2], rgba[3]
            ));
        }
        parts.push(desc);
    }
    parts.join("\n")
}

#[derive(Debug, Clone)]
pub struct GoalCheckResult {
    pub passed: bool,
    pub failures: Vec<String>,
}

/// VGRC 循环结果
#[derive(Debug, Clone)]
pub struct VgcrCycleResult {
    pub success: bool,
    pub message: String,
    pub attempts: usize,
    pub final_observation: Option<VisualObservation>,
}

/// D1 VGRC 循环结果 — 用于新的 Vision→Goal→Realize→Check 循环 API
#[derive(Debug, Clone)]
pub struct VgrcCycleResult {
    pub cycle_id: u32,
    pub goal: String,
    pub passed: bool,
    pub summary: String,
    pub screenshot_base64: Option<String>,
    pub screenshot_dimensions: Option<(u32, u32)>,
    pub analysis: Option<String>,
}

/// Realize 步骤的回调接口 — 由 DirectorRuntime 注入实际工具执行能力
pub trait RealizeExecutor: Send + Sync {
    fn execute_action(&self, action: &str) -> Result<String, String>;
}

/// 简单的基于闭包的 Realize 执行器（用于测试）
pub struct ClosureRealizeExecutor<F: Fn(&str) -> Result<String, String> + Send + Sync> {
    executor: F,
}

impl<F: Fn(&str) -> Result<String, String> + Send + Sync> ClosureRealizeExecutor<F> {
    pub fn new(f: F) -> Self {
        Self { executor: f }
    }
}

impl<F: Fn(&str) -> Result<String, String> + Send + Sync> RealizeExecutor
    for ClosureRealizeExecutor<F>
{
    fn execute_action(&self, action: &str) -> Result<String, String> {
        (self.executor)(action)
    }
}

/// 颜色匹配（允许小误差）
pub fn colors_match(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() < 0.1)
}

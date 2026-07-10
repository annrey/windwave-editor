//! Keyword constants for the rule-based planner.
//!
//! These serve a different purpose from `crate::keyword_matcher::KeywordMatcher`:
//!
//! - `KeywordMatcher` handles **topic routing** — which agent/capability should
//!   handle this request? (scene, code, review, team, etc.)
//! - This module provides **entity parsing keywords** — what specific entity
//!   name, color (with RGBA), position (with 3D offsets), or batch scope is the
//!   user describing?
//!
//! Use `KeywordMatcher` for request classification and routing; use these
//! constants for fine-grained entity attribute extraction in rule-based
//! planning.

pub(crate) const SCENE_CN_KEYWORDS: &[&str] = &["创建", "添加", "放置", "删除", "移除", "实体"];
pub(crate) const SCENE_EN_KEYWORDS: &[&str] = &[
    "create", "add", "place", "delete", "remove", "entity", "spawn",
];
pub(crate) const CODE_CN_KEYWORDS: &[&str] = &["代码", "系统", "脚本", "逻辑", "编程"];
pub(crate) const CODE_EN_KEYWORDS: &[&str] = &["code", "system", "script", "logic", "program"];
pub(crate) const ASSET_CN_KEYWORDS: &[&str] =
    &["素材", "图片", "声音", "纹理", "音乐", "音频", "模型"];
pub(crate) const ASSET_EN_KEYWORDS: &[&str] = &[
    "asset", "image", "sound", "texture", "music", "audio", "model",
];
pub(crate) const VISUAL_CN_KEYWORDS: &[&str] = &[
    "氛围", "视觉", "颜色", "粒子", "光照", "渲染", "特效", "动画",
];
pub(crate) const VISUAL_EN_KEYWORDS: &[&str] = &[
    "visual",
    "color",
    "particle",
    "light",
    "render",
    "effect",
    "animation",
];
pub(crate) const CREATE_KEYWORDS: &[&str] = &["创建", "create", "生成", "spawn", "新建", "建立"];
pub(crate) const DELETE_KEYWORDS: &[&str] =
    &["删除", "delete", "移除", "remove", "销毁", "destroy"];
pub(crate) const BATCH_KEYWORDS: &[&str] = &["批量", "batch", "全部", "all", "所有"];
pub(crate) const CHINESE_VERBS: &[&str] = &["创建", "删除", "移除", "销毁", "生成"];
pub(crate) const ENGLISH_PATTERNS: &[&str] = &[
    "create a ",
    "create an ",
    "create the ",
    "create ",
    "spawn a ",
    "spawn an ",
    "spawn the ",
    "spawn ",
    "delete a ",
    "delete an ",
    "delete the ",
    "delete ",
    "add a ",
    "add an ",
    "add the ",
    "add ",
];
pub(crate) const COLOR_KEYWORDS: &[(&[&str], &str, [f32; 4])] = &[
    (&["红色", "红", "red"], "红色", [1.0, 0.0, 0.0, 1.0]),
    (&["蓝色", "蓝", "blue"], "蓝色", [0.0, 0.0, 1.0, 1.0]),
    (&["绿色", "绿", "green"], "绿色", [0.0, 1.0, 0.0, 1.0]),
    (&["黄色", "黄", "yellow"], "黄色", [1.0, 1.0, 0.0, 1.0]),
    (&["紫色", "紫", "purple"], "紫色", [0.5, 0.0, 0.5, 1.0]),
    (&["白色", "白", "white"], "白色", [1.0, 1.0, 1.0, 1.0]),
    (&["黑色", "黑", "black"], "黑色", [0.0, 0.0, 0.0, 1.0]),
    (&["橙色", "橙", "orange"], "橙色", [1.0, 0.65, 0.0, 1.0]),
    (&["粉色", "粉", "pink"], "粉色", [1.0, 0.75, 0.8, 1.0]),
    (
        &["灰色", "灰", "gray", "grey"],
        "灰色",
        [0.5, 0.5, 0.5, 1.0],
    ),
];
pub(crate) const ENGLISH_COLOR_WORDS: &[&str] = &[
    "red", "blue", "green", "yellow", "purple", "white", "black", "orange", "pink", "gray", "grey",
];
pub(crate) const CHINESE_COLOR_PREFIXES: &[&str] = &[
    "红色", "蓝色", "绿色", "黄色", "紫色", "白色", "黑色", "橙色",
];
pub(crate) const POSITION_MARKERS: &[(&str, &str, [f32; 3])] = &[
    ("右侧", "right", [100.0, 0.0, 0.0]),
    ("右边", "right", [100.0, 0.0, 0.0]),
    ("左边", "left", [-100.0, 0.0, 0.0]),
    ("左侧", "left", [-100.0, 0.0, 0.0]),
    ("上方", "above", [0.0, 100.0, 0.0]),
    ("下方", "below", [0.0, -100.0, 0.0]),
    ("前面", "front", [0.0, 0.0, 100.0]),
    ("后面", "behind", [0.0, 0.0, -100.0]),
];

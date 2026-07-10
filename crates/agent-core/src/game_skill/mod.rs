//! Game Skill — Evolving capability system inspired by OpenGame
//!
//! Core architecture:
//! - TemplateSkill: Grows a library of project skeletons from experience
//! - DebugSkill: Maintains a living protocol of verified fixes
//! - Together they enable the agent to scaffold stable architectures
//!   and systematically repair integration errors

pub mod debug;
pub mod template;
pub mod types;
pub mod unified;

pub use debug::DebugSkill;
pub use template::TemplateSkill;
pub use types::*;
pub use unified::GameSkill;

#[cfg(test)]
mod tests;

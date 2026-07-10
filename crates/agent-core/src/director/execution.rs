//! Plan execution integration tests — split into sub-modules by concern.
//!
//! | File | Concern |
//! |------|---------|
//! | `general_integration_tests.rs` | ReAct lifecycle, acceptance, degradation, SceneBridge |
//! | `layered_context_tests.rs` | Sprint 1-C1: L0-L3 layered context |
//! | `dynamic_planner_tests.rs` | Sprint 1-A2: DynamicPlanner pattern detection + revision |
//! | `reflection_tests.rs` | Sprint 1-A3: ReflectionEngine classification + alternatives |

#[cfg(test)]
mod general_integration_tests;

#[cfg(test)]
mod layered_context_tests;

#[cfg(test)]
mod dynamic_planner_tests;

#[cfg(test)]
mod reflection_tests;

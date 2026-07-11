//! AI Resident Sandbox — first slice (`AiResidentSlice01`).
//!
//! Proves a small set of constrained residents (Merchant + Guard) can run the
//! observe → intent → action-template → permission/schedule adjudication →
//! evidence loop on top of OpenWorldSlice01 schedules and WorldClock.

use crate::open_world_verification::{
    OpenWorldVerificationBundle, VerificationBundleStatus, VerificationEvidence,
};
use crate::permission::{OperationRisk, PermissionEngine, PermissionRequirement};
use crate::world_clock::{
    AgentSchedule, ReplayLedger, ReplayLedgerEntry, ScheduleDecision, ScheduleWindow, WorldClock,
    WorldTimestamp,
};
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

/// Hard scale boundary for the sandbox (PRD §5.7).
pub const MAX_AI_RESIDENTS: usize = 20;

pub const SLICE_ID: &str = "AiResidentSlice01";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResidentRole {
    Merchant,
    Guard,
}

impl ResidentRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Merchant => "Merchant",
            Self::Guard => "Guard",
        }
    }
}

/// Fixed action templates — the only world-write path for residents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionTemplateId {
    QuotePrice,
    RestockLowRiskItem,
    IdleOffHours,
    PatrolWaypoint,
    HoldPost,
    IdleOffDuty,
}

impl ActionTemplateId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::QuotePrice => "quote_price",
            Self::RestockLowRiskItem => "restock_low_risk_item",
            Self::IdleOffHours => "idle_off_hours",
            Self::PatrolWaypoint => "patrol_waypoint",
            Self::HoldPost => "hold_post",
            Self::IdleOffDuty => "idle_off_duty",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "quote_price" => Some(Self::QuotePrice),
            "restock_low_risk_item" => Some(Self::RestockLowRiskItem),
            "idle_off_hours" => Some(Self::IdleOffHours),
            "patrol_waypoint" => Some(Self::PatrolWaypoint),
            "hold_post" => Some(Self::HoldPost),
            "idle_off_duty" => Some(Self::IdleOffDuty),
            _ => None,
        }
    }

    pub fn risk(self) -> OperationRisk {
        match self {
            Self::QuotePrice | Self::HoldPost | Self::IdleOffHours | Self::IdleOffDuty => {
                OperationRisk::Safe
            }
            Self::RestockLowRiskItem | Self::PatrolWaypoint => OperationRisk::LowRisk,
        }
    }
}

impl std::fmt::Display for ActionTemplateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResidentProfile {
    pub agent_id: String,
    pub role: ResidentRole,
    pub zone_id: String,
    /// Role-level permanent allowlist (still gated by schedule windows).
    pub allowed_actions: Vec<ActionTemplateId>,
    pub fallback_behavior: ActionTemplateId,
}

impl ResidentProfile {
    pub fn merchant(agent_id: impl Into<String>, zone_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            role: ResidentRole::Merchant,
            zone_id: zone_id.into(),
            allowed_actions: vec![
                ActionTemplateId::QuotePrice,
                ActionTemplateId::RestockLowRiskItem,
                ActionTemplateId::IdleOffHours,
            ],
            fallback_behavior: ActionTemplateId::IdleOffHours,
        }
    }

    pub fn guard(agent_id: impl Into<String>, zone_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            role: ResidentRole::Guard,
            zone_id: zone_id.into(),
            allowed_actions: vec![
                ActionTemplateId::PatrolWaypoint,
                ActionTemplateId::HoldPost,
                ActionTemplateId::IdleOffDuty,
            ],
            fallback_behavior: ActionTemplateId::IdleOffDuty,
        }
    }

    pub fn allows_template(&self, template: ActionTemplateId) -> bool {
        self.allowed_actions.contains(&template)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResidentIntent {
    pub agent_id: String,
    pub template: ActionTemplateId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdjudicationOutcome {
    Allowed {
        template: ActionTemplateId,
        window_id: Option<String>,
        risk: OperationRisk,
    },
    Denied {
        template: ActionTemplateId,
        reason: String,
    },
    Fallback {
        template: ActionTemplateId,
        reason: String,
    },
}

impl AdjudicationOutcome {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed { .. })
    }

    pub fn evidence_line(&self, agent_id: &str, tick: u64) -> String {
        match self {
            Self::Allowed {
                template,
                window_id,
                risk,
            } => format!(
                "tick={} actor={} decision=allow template={} window={} risk={:?}",
                tick,
                agent_id,
                template,
                window_id.as_deref().unwrap_or("none"),
                risk
            ),
            Self::Denied { template, reason } => format!(
                "tick={} actor={} decision=deny template={} reason={}",
                tick, agent_id, template, reason
            ),
            Self::Fallback { template, reason } => format!(
                "tick={} actor={} decision=fallback template={} reason={}",
                tick, agent_id, template, reason
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResidentObservation {
    pub agent_id: String,
    pub role: ResidentRole,
    pub zone_id: String,
    pub schedule: ScheduleDecision,
    pub active_template: Option<ActionTemplateId>,
}

impl ResidentObservation {
    pub fn scene_index_line(&self) -> String {
        let window = self.schedule.window_id.as_deref().unwrap_or("none");
        let activity = if self.schedule.is_active {
            "active"
        } else {
            "inactive"
        };
        let allowed = if self.schedule.allowed_actions.is_empty() {
            "none".to_string()
        } else {
            self.schedule.allowed_actions.join(",")
        };
        let template = self
            .active_template
            .map(|t| t.as_str())
            .unwrap_or("none");
        format!(
            "agent_id={} role={} zone={} schedule={} window={} allowed={} template={}",
            self.agent_id,
            self.role.as_str(),
            self.zone_id,
            activity,
            window,
            allowed,
            template
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AiResidentError {
    #[error("AI resident count {count} exceeds sandbox max {max}")]
    ScaleLimitExceeded { count: usize, max: usize },
    #[error("unknown resident '{0}'")]
    UnknownResident(String),
    #[error("resident '{0}' has no schedule")]
    MissingSchedule(String),
}

/// Deterministic core harness for AiResidentSlice01.
#[derive(Debug, Clone)]
pub struct AiResidentSandbox {
    pub slice_id: String,
    pub profiles: BTreeMap<String, ResidentProfile>,
    pub schedules: BTreeMap<String, AgentSchedule>,
    pub clock: WorldClock,
    pub ledger: ReplayLedger,
    pub permission_engine: PermissionEngine,
    pub active_templates: BTreeMap<String, ActionTemplateId>,
    pub agent_events: Vec<String>,
    entry_counter: u64,
}

impl AiResidentSandbox {
    pub fn from_profiles(
        profiles: Vec<ResidentProfile>,
        schedules: Vec<AgentSchedule>,
        clock: WorldClock,
    ) -> Result<Self, AiResidentError> {
        if profiles.len() > MAX_AI_RESIDENTS {
            return Err(AiResidentError::ScaleLimitExceeded {
                count: profiles.len(),
                max: MAX_AI_RESIDENTS,
            });
        }

        let profiles: BTreeMap<_, _> = profiles
            .into_iter()
            .map(|profile| (profile.agent_id.clone(), profile))
            .collect();
        let schedules: BTreeMap<_, _> = schedules
            .into_iter()
            .map(|schedule| (schedule.agent_id.clone(), schedule))
            .collect();

        Ok(Self {
            slice_id: SLICE_ID.into(),
            profiles,
            schedules,
            clock,
            ledger: ReplayLedger::default(),
            permission_engine: PermissionEngine::new(),
            active_templates: BTreeMap::new(),
            agent_events: Vec::new(),
            entry_counter: 0,
        })
    }

    /// Merchant dock + Guard camp patrol on OpenWorldSlice01 island.
    pub fn slice01_fixture() -> Self {
        Self::from_profiles(
            vec![
                ResidentProfile::merchant("merchant_01", "spawn_zone"),
                ResidentProfile::guard("guard_01", "camp_zone"),
            ],
            vec![
                AgentSchedule::new("merchant_01").with_window(ScheduleWindow::wall_time_daily(
                    "shop_hours",
                    9,
                    0,
                    21,
                    0,
                    ["quote_price", "restock_low_risk_item"],
                )),
                AgentSchedule::new("guard_01").with_window(ScheduleWindow::wall_time_daily(
                    "patrol_hours",
                    6,
                    0,
                    22,
                    0,
                    ["patrol_waypoint", "hold_post"],
                )),
            ],
            WorldClock::frozen_at(Utc.with_ymd_and_hms(2026, 7, 5, 12, 0, 0).unwrap()),
        )
        .expect("slice01 fixture is within scale limit")
    }

    pub fn freeze_at(&mut self, wall_time: DateTime<Utc>) {
        let tick = self.clock.tick;
        let sim_time_ms = self.clock.sim_time_ms;
        self.clock = WorldClock::frozen_at(wall_time);
        self.clock.tick = tick;
        self.clock.sim_time_ms = sim_time_ms;
    }

    /// Convenience for Bevy / callers that should not depend on chrono directly.
    pub fn freeze_to_hour(&mut self, hour: u32) {
        let hour = hour.min(23);
        self.freeze_at(Utc.with_ymd_and_hms(2026, 7, 5, hour, 0, 0).unwrap());
    }

    pub fn advance_tick(&mut self) -> Result<WorldTimestamp, crate::WorldClockError> {
        self.clock.advance_one_tick()
    }

    pub fn observe(&self, agent_id: &str) -> Result<ResidentObservation, AiResidentError> {
        let profile = self
            .profiles
            .get(agent_id)
            .ok_or_else(|| AiResidentError::UnknownResident(agent_id.into()))?;
        let schedule = self
            .schedules
            .get(agent_id)
            .ok_or_else(|| AiResidentError::MissingSchedule(agent_id.into()))?;
        Ok(ResidentObservation {
            agent_id: agent_id.into(),
            role: profile.role,
            zone_id: profile.zone_id.clone(),
            schedule: schedule.decision_at(&self.clock.timestamp()),
            active_template: self.active_templates.get(agent_id).copied(),
        })
    }

    pub fn observe_all(&self) -> Vec<ResidentObservation> {
        self.profiles
            .keys()
            .filter_map(|agent_id| self.observe(agent_id).ok())
            .collect()
    }

    /// Schedule-driven intent: pick first allowed window action, else fallback.
    pub fn schedule_driven_intent(
        &self,
        agent_id: &str,
    ) -> Result<ResidentIntent, AiResidentError> {
        let profile = self
            .profiles
            .get(agent_id)
            .ok_or_else(|| AiResidentError::UnknownResident(agent_id.into()))?;
        let decision = self
            .schedules
            .get(agent_id)
            .ok_or_else(|| AiResidentError::MissingSchedule(agent_id.into()))?
            .decision_at(&self.clock.timestamp());

        let template = decision
            .allowed_actions
            .iter()
            .find_map(|action| ActionTemplateId::parse(action))
            .filter(|template| profile.allows_template(*template))
            .unwrap_or(profile.fallback_behavior);

        Ok(ResidentIntent {
            agent_id: agent_id.into(),
            template,
        })
    }

    pub fn adjudicate(&mut self, intent: &ResidentIntent) -> Result<AdjudicationOutcome, AiResidentError> {
        let profile = self
            .profiles
            .get(&intent.agent_id)
            .cloned()
            .ok_or_else(|| AiResidentError::UnknownResident(intent.agent_id.clone()))?;
        let schedule = self
            .schedules
            .get(&intent.agent_id)
            .ok_or_else(|| AiResidentError::MissingSchedule(intent.agent_id.clone()))?;
        let decision = schedule.decision_at(&self.clock.timestamp());
        let tick = self.clock.tick;

        if !profile.allows_template(intent.template) {
            let outcome = AdjudicationOutcome::Denied {
                template: intent.template,
                reason: format!(
                    "template '{}' not in allowed_actions for {}",
                    intent.template,
                    profile.role.as_str()
                ),
            };
            self.record_outcome(&intent.agent_id, &outcome, tick);
            return Ok(outcome);
        }

        let is_idle_fallback = intent.template == profile.fallback_behavior;
        if !decision.is_active && is_idle_fallback {
            let outcome = AdjudicationOutcome::Fallback {
                template: intent.template,
                reason: "outside schedule window; applying fallback_behavior".into(),
            };
            self.active_templates
                .insert(intent.agent_id.clone(), intent.template);
            self.record_outcome(&intent.agent_id, &outcome, tick);
            return Ok(outcome);
        }

        if !decision.allows(intent.template.as_str()) {
            let outcome = if decision.is_active {
                AdjudicationOutcome::Denied {
                    template: intent.template,
                    reason: format!(
                        "template '{}' not allowed in active window '{}'",
                        intent.template,
                        decision.window_id.as_deref().unwrap_or("none")
                    ),
                }
            } else {
                AdjudicationOutcome::Denied {
                    template: intent.template,
                    reason: format!(
                        "outside schedule window; '{}' rejected (fallback={})",
                        intent.template, profile.fallback_behavior
                    ),
                }
            };
            self.record_outcome(&intent.agent_id, &outcome, tick);
            return Ok(outcome);
        }

        match self.permission_engine.decide_for_plan(intent.template.risk()) {
            PermissionRequirement::AutoApproved => {
                let outcome = AdjudicationOutcome::Allowed {
                    template: intent.template,
                    window_id: decision.window_id.clone(),
                    risk: intent.template.risk(),
                };
                self.active_templates
                    .insert(intent.agent_id.clone(), intent.template);
                self.record_outcome(&intent.agent_id, &outcome, tick);
                Ok(outcome)
            }
            PermissionRequirement::NeedUserConfirmation { risk, reason } => {
                let outcome = AdjudicationOutcome::Denied {
                    template: intent.template,
                    reason: format!("permission requires confirmation for {:?}: {}", risk, reason),
                };
                self.record_outcome(&intent.agent_id, &outcome, tick);
                Ok(outcome)
            }
            PermissionRequirement::Forbidden { reason } => {
                let outcome = AdjudicationOutcome::Denied {
                    template: intent.template,
                    reason: format!("permission forbidden: {}", reason),
                };
                self.record_outcome(&intent.agent_id, &outcome, tick);
                Ok(outcome)
            }
        }
    }

    /// Run one schedule-driven tick for every resident.
    pub fn tick_all_schedule_driven(&mut self) -> Result<Vec<AdjudicationOutcome>, AiResidentError> {
        let agent_ids: Vec<String> = self.profiles.keys().cloned().collect();
        let mut outcomes = Vec::with_capacity(agent_ids.len());
        for agent_id in agent_ids {
            let intent = self.schedule_driven_intent(&agent_id)?;
            outcomes.push(self.adjudicate(&intent)?);
        }
        Ok(outcomes)
    }

    pub fn scene_index_observations(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "world_clock wall_time={} sim_time_ms={} tick={} clock_mode={:?}",
            self.clock.wall_time.to_rfc3339(),
            self.clock.sim_time_ms,
            self.clock.tick,
            self.clock.mode
        )];
        lines.extend(
            self.observe_all()
                .into_iter()
                .map(|obs| obs.scene_index_line()),
        );
        lines
    }

    pub fn schedule_decision_lines(&self) -> Vec<String> {
        self.schedules
            .values()
            .map(|schedule| {
                let decision = schedule.decision_at(&self.clock.timestamp());
                let window = decision.window_id.as_deref().unwrap_or("none");
                let activity = if decision.is_active {
                    "active"
                } else {
                    "inactive"
                };
                let allowed = if decision.allowed_actions.is_empty() {
                    "none".to_string()
                } else {
                    decision.allowed_actions.join(",")
                };
                format!(
                    "{} {} window={} allowed={}",
                    decision.agent_id, activity, window, allowed
                )
            })
            .collect()
    }

    pub fn to_verification_bundle(&self) -> OpenWorldVerificationBundle {
        let has_schedule = !self.schedule_decision_lines().is_empty();
        let has_agent_obs = self
            .scene_index_observations()
            .iter()
            .any(|line| line.starts_with("agent_id="));
        let passed = has_schedule && has_agent_obs && !self.agent_events.is_empty();

        OpenWorldVerificationBundle {
            id: format!("verification_{}", self.slice_id),
            plan_id: "open_world_slice01".into(),
            scenario_id: "playtest_ai_resident_slice01".into(),
            status: if passed {
                VerificationBundleStatus::Passed
            } else {
                VerificationBundleStatus::Failed
            },
            playtest: crate::open_world_verification::PlaytestVerificationSummary {
                scenario_id: "playtest_ai_resident_slice01".into(),
                passed,
                completed_steps: self.agent_events.len(),
                final_quest_state: "N/A".into(),
                failed_step_index: None,
                failed_step_label: None,
                failure_reason: None,
                suggested_fix: None,
            },
            goals: vec![],
            evidence: VerificationEvidence {
                runtime_events: self
                    .ledger
                    .entries
                    .iter()
                    .map(|entry| {
                        format!(
                            "tick={} {} {}",
                            entry.timestamp.tick, entry.event_kind, entry.summary
                        )
                    })
                    .collect(),
                playtest_events: self.agent_events.clone(),
                time_evidence: vec![format!(
                    "wall_time={} sim_time_ms={} tick={} clock_mode={:?}",
                    self.clock.wall_time.to_rfc3339(),
                    self.clock.sim_time_ms,
                    self.clock.tick,
                    self.clock.mode
                )],
                schedule_decisions: self.schedule_decision_lines(),
                performance_evidence: vec![
                    format!("resident_count={}", self.profiles.len()),
                    format!("max_ai_residents={}", MAX_AI_RESIDENTS),
                    format!("ledger_entries={}", self.ledger.entries.len()),
                ],
                scene_index_observations: self.scene_index_observations(),
                screenshot_paths: Vec::new(),
                visual_check_evidence: Vec::new(),
                director_events: Vec::new(),
                engine_events: Vec::new(),
                agent_events: self.agent_events.clone(),
            },
        }
    }

    /// Automated playtest covering PRD §5 success criteria 1–5, 7.
    pub fn run_slice01_playtest() -> OpenWorldVerificationBundle {
        let mut sandbox = Self::slice01_fixture();

        // 1. Shop hours: quote_price allowed
        sandbox.freeze_at(Utc.with_ymd_and_hms(2026, 7, 5, 12, 0, 0).unwrap());
        let _ = sandbox.advance_tick();
        let open = sandbox
            .adjudicate(&ResidentIntent {
                agent_id: "merchant_01".into(),
                template: ActionTemplateId::QuotePrice,
            })
            .expect("merchant exists");
        assert!(open.is_allowed(), "merchant should quote in shop hours");

        // 1b. Off hours: quote_price denied
        sandbox.freeze_at(Utc.with_ymd_and_hms(2026, 7, 5, 23, 0, 0).unwrap());
        let _ = sandbox.advance_tick();
        let closed = sandbox
            .adjudicate(&ResidentIntent {
                agent_id: "merchant_01".into(),
                template: ActionTemplateId::QuotePrice,
            })
            .expect("merchant exists");
        assert!(
            matches!(closed, AdjudicationOutcome::Denied { .. }),
            "merchant quote outside hours must deny"
        );

        // 2. Patrol hours: patrol allowed
        sandbox.freeze_at(Utc.with_ymd_and_hms(2026, 7, 5, 10, 0, 0).unwrap());
        let _ = sandbox.advance_tick();
        let patrol = sandbox
            .adjudicate(&ResidentIntent {
                agent_id: "guard_01".into(),
                template: ActionTemplateId::PatrolWaypoint,
            })
            .expect("guard exists");
        assert!(patrol.is_allowed(), "guard should patrol on duty");

        // 2b. Off duty: patrol denied / fallback evidence
        sandbox.freeze_at(Utc.with_ymd_and_hms(2026, 7, 5, 23, 30, 0).unwrap());
        let _ = sandbox.advance_tick();
        let off_duty = sandbox
            .adjudicate(&ResidentIntent {
                agent_id: "guard_01".into(),
                template: ActionTemplateId::PatrolWaypoint,
            })
            .expect("guard exists");
        assert!(
            matches!(
                off_duty,
                AdjudicationOutcome::Denied { .. } | AdjudicationOutcome::Fallback { .. }
            ),
            "guard patrol off duty must deny or fallback"
        );
        let fallback = sandbox
            .adjudicate(&ResidentIntent {
                agent_id: "guard_01".into(),
                template: ActionTemplateId::IdleOffDuty,
            })
            .expect("guard exists");
        assert!(
            matches!(
                fallback,
                AdjudicationOutcome::Fallback { .. } | AdjudicationOutcome::Allowed { .. }
            ),
            "guard idle_off_duty should be recorded as fallback/allow"
        );

        // 3. Unauthorized template
        sandbox.freeze_at(Utc.with_ymd_and_hms(2026, 7, 5, 12, 0, 0).unwrap());
        let _ = sandbox.advance_tick();
        let unauthorized = sandbox
            .adjudicate(&ResidentIntent {
                agent_id: "merchant_01".into(),
                template: ActionTemplateId::PatrolWaypoint,
            })
            .expect("merchant exists");
        assert!(
            matches!(unauthorized, AdjudicationOutcome::Denied { .. }),
            "merchant must not patrol"
        );

        // 4–5. Observations + bundle
        let obs = sandbox.observe("merchant_01").expect("merchant");
        assert_eq!(obs.role, ResidentRole::Merchant);
        assert!(!obs.zone_id.is_empty());

        let bundle = sandbox.to_verification_bundle();
        assert_eq!(bundle.status, VerificationBundleStatus::Passed);
        assert!(!bundle.evidence.schedule_decisions.is_empty());
        assert!(!bundle.evidence.agent_events.is_empty());
        assert!(bundle
            .evidence
            .agent_events
            .iter()
            .any(|e| e.contains("decision=deny")));
        assert!(bundle
            .evidence
            .scene_index_observations
            .iter()
            .any(|o| o.contains("agent_id=merchant_01")));
        assert!(bundle
            .evidence
            .scene_index_observations
            .iter()
            .any(|o| o.contains("agent_id=guard_01")));

        bundle
    }

    fn record_outcome(&mut self, agent_id: &str, outcome: &AdjudicationOutcome, tick: u64) {
        let line = outcome.evidence_line(agent_id, tick);
        self.agent_events.push(line.clone());
        self.entry_counter = self.entry_counter.saturating_add(1);
        let event_kind = match outcome {
            AdjudicationOutcome::Allowed { .. } => "resident_allow",
            AdjudicationOutcome::Denied { .. } => "resident_deny",
            AdjudicationOutcome::Fallback { .. } => "resident_fallback",
        };
        self.ledger.push(ReplayLedgerEntry::new(
            format!("ai_resident_{}", self.entry_counter),
            self.clock.timestamp(),
            agent_id,
            event_kind,
            line,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice01_playtest_meets_prd_success_criteria() {
        let bundle = AiResidentSandbox::run_slice01_playtest();
        assert_eq!(bundle.status, VerificationBundleStatus::Passed);
        let markdown = bundle.to_markdown();
        assert!(markdown.contains("Schedule Decisions"));
        assert!(markdown.contains("Agent Events") || markdown.contains("decision=deny"));
        assert!(markdown.contains("merchant_01") || markdown.contains("agent_id=merchant_01"));
    }

    #[test]
    fn merchant_quote_allowed_in_shop_hours_denied_outside() {
        let mut sandbox = AiResidentSandbox::slice01_fixture();

        sandbox.freeze_at(Utc.with_ymd_and_hms(2026, 7, 5, 12, 0, 0).unwrap());
        let allowed = sandbox
            .adjudicate(&ResidentIntent {
                agent_id: "merchant_01".into(),
                template: ActionTemplateId::QuotePrice,
            })
            .unwrap();
        assert!(matches!(allowed, AdjudicationOutcome::Allowed { .. }));

        sandbox.freeze_at(Utc.with_ymd_and_hms(2026, 7, 5, 23, 0, 0).unwrap());
        let denied = sandbox
            .adjudicate(&ResidentIntent {
                agent_id: "merchant_01".into(),
                template: ActionTemplateId::QuotePrice,
            })
            .unwrap();
        assert!(matches!(denied, AdjudicationOutcome::Denied { .. }));
        assert!(sandbox
            .ledger
            .entries
            .iter()
            .any(|e| e.event_kind == "resident_deny" && e.actor == "merchant_01"));
    }

    #[test]
    fn guard_patrol_on_duty_and_fallback_off_duty() {
        let mut sandbox = AiResidentSandbox::slice01_fixture();

        sandbox.freeze_at(Utc.with_ymd_and_hms(2026, 7, 5, 8, 0, 0).unwrap());
        let on_duty = sandbox
            .adjudicate(&ResidentIntent {
                agent_id: "guard_01".into(),
                template: ActionTemplateId::PatrolWaypoint,
            })
            .unwrap();
        assert!(on_duty.is_allowed());

        sandbox.freeze_at(Utc.with_ymd_and_hms(2026, 7, 5, 23, 0, 0).unwrap());
        let denied = sandbox
            .adjudicate(&ResidentIntent {
                agent_id: "guard_01".into(),
                template: ActionTemplateId::PatrolWaypoint,
            })
            .unwrap();
        assert!(matches!(denied, AdjudicationOutcome::Denied { .. }));

        let fallback = sandbox
            .adjudicate(&ResidentIntent {
                agent_id: "guard_01".into(),
                template: ActionTemplateId::IdleOffDuty,
            })
            .unwrap();
        assert!(matches!(fallback, AdjudicationOutcome::Fallback { .. }));
        assert!(sandbox
            .agent_events
            .iter()
            .any(|e| e.contains("decision=fallback") && e.contains("guard_01")));
    }

    #[test]
    fn unauthorized_action_is_denied_and_audited() {
        let mut sandbox = AiResidentSandbox::slice01_fixture();
        sandbox.freeze_at(Utc.with_ymd_and_hms(2026, 7, 5, 12, 0, 0).unwrap());

        let denied = sandbox
            .adjudicate(&ResidentIntent {
                agent_id: "merchant_01".into(),
                template: ActionTemplateId::PatrolWaypoint,
            })
            .unwrap();

        assert!(matches!(denied, AdjudicationOutcome::Denied { .. }));
        assert!(sandbox.agent_events.iter().any(|e| {
            e.contains("decision=deny")
                && e.contains("merchant_01")
                && e.contains("patrol_waypoint")
        }));
    }

    #[test]
    fn scene_index_observations_include_agent_state() {
        let mut sandbox = AiResidentSandbox::slice01_fixture();
        sandbox.freeze_at(Utc.with_ymd_and_hms(2026, 7, 5, 12, 0, 0).unwrap());
        sandbox.tick_all_schedule_driven().unwrap();

        let lines = sandbox.scene_index_observations();
        assert!(lines.iter().any(|l| l.contains("agent_id=merchant_01")
            && l.contains("role=Merchant")
            && l.contains("zone=spawn_zone")));
        assert!(lines.iter().any(|l| l.contains("agent_id=guard_01")
            && l.contains("role=Guard")
            && l.contains("zone=camp_zone")));
    }

    #[test]
    fn scale_limit_rejects_more_than_twenty_residents() {
        let profiles: Vec<_> = (0..21)
            .map(|i| ResidentProfile::merchant(format!("merchant_{i:02}"), "spawn_zone"))
            .collect();
        let schedules: Vec<_> = profiles
            .iter()
            .map(|p| AgentSchedule::new(&p.agent_id))
            .collect();

        let err = AiResidentSandbox::from_profiles(
            profiles,
            schedules,
            WorldClock::frozen_at(Utc.with_ymd_and_hms(2026, 7, 5, 12, 0, 0).unwrap()),
        )
        .unwrap_err();

        assert!(matches!(
            err,
            AiResidentError::ScaleLimitExceeded {
                count: 21,
                max: MAX_AI_RESIDENTS
            }
        ));
    }

    #[test]
    fn schedule_driven_intent_picks_window_or_fallback() {
        let mut sandbox = AiResidentSandbox::slice01_fixture();

        sandbox.freeze_at(Utc.with_ymd_and_hms(2026, 7, 5, 12, 0, 0).unwrap());
        let open = sandbox.schedule_driven_intent("merchant_01").unwrap();
        assert_eq!(open.template, ActionTemplateId::QuotePrice);

        sandbox.freeze_at(Utc.with_ymd_and_hms(2026, 7, 5, 23, 0, 0).unwrap());
        let closed = sandbox.schedule_driven_intent("merchant_01").unwrap();
        assert_eq!(closed.template, ActionTemplateId::IdleOffHours);
    }
}

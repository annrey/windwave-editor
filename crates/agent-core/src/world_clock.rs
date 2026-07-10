//! Shared world clock model for playable-world runtime, schedules, and replay.
//!
//! This module keeps core time semantics independent from Bevy `Time`, so
//! tests and headless playtest runners can freeze, advance, and replay time.

use chrono::{DateTime, Duration, Timelike, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldTimestamp {
    pub wall_time: DateTime<Utc>,
    pub sim_time_ms: u64,
    pub tick: u64,
}

impl WorldTimestamp {
    pub fn at_wall_time(wall_time: DateTime<Utc>) -> Self {
        Self {
            wall_time,
            sim_time_ms: 0,
            tick: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorldClockMode {
    Frozen,
    Manual,
    Realtime,
    Replay,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldClock {
    pub wall_time: DateTime<Utc>,
    pub sim_time_ms: u64,
    pub tick: u64,
    pub time_scale: f32,
    pub mode: WorldClockMode,
    pub last_advanced_wall_time: DateTime<Utc>,
}

impl WorldClock {
    pub fn frozen_at(wall_time: DateTime<Utc>) -> Self {
        Self::new(wall_time, WorldClockMode::Frozen, 0.0)
    }

    pub fn manual_at(wall_time: DateTime<Utc>) -> Self {
        Self::new(wall_time, WorldClockMode::Manual, 0.0)
    }

    pub fn realtime_at(wall_time: DateTime<Utc>, time_scale: f32) -> Self {
        Self::new(wall_time, WorldClockMode::Realtime, time_scale)
    }

    fn new(wall_time: DateTime<Utc>, mode: WorldClockMode, time_scale: f32) -> Self {
        Self {
            wall_time,
            sim_time_ms: 0,
            tick: 0,
            time_scale,
            mode,
            last_advanced_wall_time: wall_time,
        }
    }

    pub fn timestamp(&self) -> WorldTimestamp {
        WorldTimestamp {
            wall_time: self.wall_time,
            sim_time_ms: self.sim_time_ms,
            tick: self.tick,
        }
    }

    pub fn advance_one_tick(&mut self) -> Result<WorldTimestamp, WorldClockError> {
        if self.mode == WorldClockMode::Replay {
            return Err(WorldClockError::ReplayClockIsReadOnly);
        }

        self.tick = self.tick.saturating_add(1);
        Ok(self.timestamp())
    }

    pub fn advance_by_sim_duration(
        &mut self,
        duration_ms: u64,
    ) -> Result<WorldTimestamp, WorldClockError> {
        if self.mode == WorldClockMode::Replay {
            return Err(WorldClockError::ReplayClockIsReadOnly);
        }

        self.sim_time_ms = self.sim_time_ms.saturating_add(duration_ms);
        Ok(self.timestamp())
    }

    pub fn advance_by_wall_duration(
        &mut self,
        duration_ms: u64,
        policy: &TimePolicy,
    ) -> Result<WorldTimestamp, WorldClockError> {
        if self.mode == WorldClockMode::Frozen {
            return Err(WorldClockError::FrozenCannotAdvanceFromWallDuration);
        }
        if duration_ms > policy.max_offline_duration_ms {
            return Err(WorldClockError::OfflineDurationExceedsPolicy {
                requested_ms: duration_ms,
                max_ms: policy.max_offline_duration_ms,
            });
        }

        let next_wall_time = self.wall_time + Duration::milliseconds(duration_ms as i64);
        self.advance_to_wall_time(next_wall_time, policy)
    }

    pub fn advance_to_wall_time(
        &mut self,
        wall_time: DateTime<Utc>,
        policy: &TimePolicy,
    ) -> Result<WorldTimestamp, WorldClockError> {
        if self.mode == WorldClockMode::Replay {
            return Err(WorldClockError::ReplayClockIsReadOnly);
        }
        if wall_time < self.wall_time {
            return Err(WorldClockError::WallTimeRegression);
        }

        let elapsed_ms = wall_time
            .signed_duration_since(self.wall_time)
            .num_milliseconds() as u64;
        if elapsed_ms > policy.max_offline_duration_ms {
            return Err(WorldClockError::OfflineDurationExceedsPolicy {
                requested_ms: elapsed_ms,
                max_ms: policy.max_offline_duration_ms,
            });
        }

        if self.mode == WorldClockMode::Realtime {
            let scaled_ms = (elapsed_ms as f64 * self.time_scale as f64).round() as u64;
            self.sim_time_ms = self.sim_time_ms.saturating_add(scaled_ms);
        }

        self.wall_time = wall_time;
        self.last_advanced_wall_time = wall_time;
        self.tick = self.tick.saturating_add(1);
        Ok(self.timestamp())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimePolicy {
    pub allow_offline_progression: bool,
    pub max_offline_duration_ms: u64,
    pub default_time_scale: f32,
    pub calendar_origin_wall_time: Option<DateTime<Utc>>,
    pub day_length_sim_ms: u64,
    pub high_risk_requires_approval: bool,
}

impl Default for TimePolicy {
    fn default() -> Self {
        Self {
            allow_offline_progression: false,
            max_offline_duration_ms: 86_400_000,
            default_time_scale: 1.0,
            calendar_origin_wall_time: None,
            day_length_sim_ms: 1_200_000,
            high_risk_requires_approval: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CalendarEventState {
    Scheduled,
    Active,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: String,
    pub label: String,
    pub starts_at: WorldTimestamp,
    pub ends_at: WorldTimestamp,
}

impl CalendarEvent {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        starts_at: WorldTimestamp,
        ends_at: WorldTimestamp,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            starts_at,
            ends_at,
        }
    }

    pub fn state_at(&self, timestamp: &WorldTimestamp) -> CalendarEventState {
        if timestamp.wall_time < self.starts_at.wall_time {
            CalendarEventState::Scheduled
        } else if timestamp.wall_time <= self.ends_at.wall_time {
            CalendarEventState::Active
        } else {
            CalendarEventState::Expired
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSchedule {
    pub agent_id: String,
    pub windows: Vec<ScheduleWindow>,
}

impl AgentSchedule {
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            windows: Vec::new(),
        }
    }

    pub fn with_window(mut self, window: ScheduleWindow) -> Self {
        self.windows.push(window);
        self
    }

    pub fn decision_at(&self, timestamp: &WorldTimestamp) -> ScheduleDecision {
        for window in &self.windows {
            if window.contains(timestamp) {
                return ScheduleDecision {
                    agent_id: self.agent_id.clone(),
                    window_id: Some(window.id.clone()),
                    is_active: true,
                    allowed_actions: window.allowed_actions.clone(),
                };
            }
        }

        ScheduleDecision {
            agent_id: self.agent_id.clone(),
            window_id: None,
            is_active: false,
            allowed_actions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleWindow {
    pub id: String,
    pub start_minute_of_day: u16,
    pub end_minute_of_day: u16,
    pub allowed_actions: Vec<String>,
}

impl ScheduleWindow {
    pub fn wall_time_daily<I, S>(
        id: impl Into<String>,
        start_hour: u32,
        start_minute: u32,
        end_hour: u32,
        end_minute: u32,
        allowed_actions: I,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            id: id.into(),
            start_minute_of_day: (start_hour * 60 + start_minute) as u16,
            end_minute_of_day: (end_hour * 60 + end_minute) as u16,
            allowed_actions: allowed_actions.into_iter().map(Into::into).collect(),
        }
    }

    pub fn contains(&self, timestamp: &WorldTimestamp) -> bool {
        let minute_of_day = (timestamp.wall_time.hour() * 60 + timestamp.wall_time.minute()) as u16;

        if self.start_minute_of_day <= self.end_minute_of_day {
            minute_of_day >= self.start_minute_of_day && minute_of_day < self.end_minute_of_day
        } else {
            minute_of_day >= self.start_minute_of_day || minute_of_day < self.end_minute_of_day
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleDecision {
    pub agent_id: String,
    pub window_id: Option<String>,
    pub is_active: bool,
    pub allowed_actions: Vec<String>,
}

impl ScheduleDecision {
    pub fn allows(&self, action: &str) -> bool {
        self.allowed_actions.iter().any(|allowed| allowed == action)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfflineProgressionPolicy {
    pub allow_offline_progression: bool,
    pub max_offline_duration_ms: u64,
    pub high_risk_requires_approval: bool,
}

impl OfflineProgressionPolicy {
    pub fn evaluate(
        &self,
        last_seen: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> OfflineProgressionReport {
        let actual_elapsed_ms = if now >= last_seen {
            now.signed_duration_since(last_seen).num_milliseconds() as u64
        } else {
            0
        };
        let applied_elapsed_ms = if self.allow_offline_progression {
            actual_elapsed_ms.min(self.max_offline_duration_ms)
        } else {
            0
        };

        OfflineProgressionReport {
            actual_elapsed_ms,
            applied_elapsed_ms,
            truncated: applied_elapsed_ms < actual_elapsed_ms,
            blocked_high_risk_actions: self.high_risk_requires_approval,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfflineProgressionReport {
    pub actual_elapsed_ms: u64,
    pub applied_elapsed_ms: u64,
    pub truncated: bool,
    pub blocked_high_risk_actions: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayLedger {
    pub entries: Vec<ReplayLedgerEntry>,
}

impl ReplayLedger {
    pub fn push(&mut self, entry: ReplayLedgerEntry) {
        self.entries.push(entry);
    }

    pub fn by_tick(&self, tick: u64) -> Option<&ReplayLedgerEntry> {
        self.entries
            .iter()
            .find(|entry| entry.timestamp.tick == tick)
    }

    pub fn by_wall_time(&self, wall_time: DateTime<Utc>) -> Option<&ReplayLedgerEntry> {
        self.entries
            .iter()
            .find(|entry| entry.timestamp.wall_time == wall_time)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayLedgerEntry {
    pub entry_id: String,
    pub timestamp: WorldTimestamp,
    pub actor: String,
    pub event_kind: String,
    pub summary: String,
    pub evidence_refs: Vec<String>,
}

impl ReplayLedgerEntry {
    pub fn new(
        entry_id: impl Into<String>,
        timestamp: WorldTimestamp,
        actor: impl Into<String>,
        event_kind: impl Into<String>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            entry_id: entry_id.into(),
            timestamp,
            actor: actor.into(),
            event_kind: event_kind.into(),
            summary: summary.into(),
            evidence_refs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorldClockError {
    #[error("frozen clock cannot advance from wall duration")]
    FrozenCannotAdvanceFromWallDuration,
    #[error("world clock cannot move wall_time backwards")]
    WallTimeRegression,
    #[error("replay clock is read-only")]
    ReplayClockIsReadOnly,
    #[error("offline duration {requested_ms}ms exceeds policy max {max_ms}ms")]
    OfflineDurationExceedsPolicy { requested_ms: u64, max_ms: u64 },
}

use agent_core::{
    AgentSchedule, CalendarEvent, CalendarEventState, OfflineProgressionPolicy, ReplayLedger,
    ReplayLedgerEntry, ScheduleWindow, TimePolicy, WorldClock, WorldClockMode, WorldTimestamp,
};
use chrono::{DateTime, TimeZone, Utc};

fn utc(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
        .unwrap()
}

#[test]
fn world_clock_round_trips_through_json() {
    let clock = WorldClock::frozen_at(utc(2026, 7, 5, 12, 0, 0));

    let json = serde_json::to_string(&clock).unwrap();
    let restored: WorldClock = serde_json::from_str(&json).unwrap();

    assert_eq!(restored, clock);
    assert_eq!(restored.mode, WorldClockMode::Frozen);
    assert_eq!(restored.timestamp().tick, 0);
}

#[test]
fn frozen_clock_rejects_wall_time_advancement_but_allows_manual_ticks() {
    let mut clock = WorldClock::frozen_at(utc(2026, 7, 5, 12, 0, 0));

    let err = clock
        .advance_by_wall_duration(1_000, &TimePolicy::default())
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "frozen clock cannot advance from wall duration"
    );

    clock.advance_one_tick().unwrap();
    clock.advance_by_sim_duration(500).unwrap();

    assert_eq!(clock.timestamp().tick, 1);
    assert_eq!(clock.timestamp().sim_time_ms, 500);
    assert_eq!(clock.timestamp().wall_time, utc(2026, 7, 5, 12, 0, 0));
}

#[test]
fn manual_clock_rejects_time_regression() {
    let mut clock = WorldClock::manual_at(utc(2026, 7, 5, 12, 0, 0));

    clock
        .advance_to_wall_time(utc(2026, 7, 5, 12, 1, 0), &TimePolicy::default())
        .unwrap();
    let err = clock
        .advance_to_wall_time(utc(2026, 7, 5, 12, 0, 30), &TimePolicy::default())
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "world clock cannot move wall_time backwards"
    );
}

#[test]
fn realtime_clock_uses_time_scale_for_sim_time() {
    let mut clock = WorldClock::realtime_at(utc(2026, 7, 5, 12, 0, 0), 2.0);

    clock
        .advance_to_wall_time(utc(2026, 7, 5, 12, 0, 10), &TimePolicy::default())
        .unwrap();

    assert_eq!(clock.timestamp().wall_time, utc(2026, 7, 5, 12, 0, 10));
    assert_eq!(clock.timestamp().sim_time_ms, 20_000);
    assert_eq!(clock.timestamp().tick, 1);
}

#[test]
fn calendar_event_reports_scheduled_active_and_expired() {
    let event = CalendarEvent::new(
        "night_patrol_window",
        "Night Patrol",
        WorldTimestamp::at_wall_time(utc(2026, 7, 5, 20, 0, 0)),
        WorldTimestamp::at_wall_time(utc(2026, 7, 5, 22, 0, 0)),
    );

    assert_eq!(
        event.state_at(&WorldTimestamp::at_wall_time(utc(2026, 7, 5, 19, 59, 0))),
        CalendarEventState::Scheduled
    );
    assert_eq!(
        event.state_at(&WorldTimestamp::at_wall_time(utc(2026, 7, 5, 21, 0, 0))),
        CalendarEventState::Active
    );
    assert_eq!(
        event.state_at(&WorldTimestamp::at_wall_time(utc(2026, 7, 5, 22, 1, 0))),
        CalendarEventState::Expired
    );
}

#[test]
fn agent_schedule_decides_allowed_actions_from_window() {
    let schedule = AgentSchedule::new("merchant_01").with_window(ScheduleWindow::wall_time_daily(
        "shop_hours",
        9,
        0,
        21,
        0,
        ["quote_price", "restock_low_risk_item"],
    ));

    let open = schedule.decision_at(&WorldTimestamp::at_wall_time(utc(2026, 7, 5, 12, 0, 0)));
    assert!(open.is_active);
    assert!(open.allows("quote_price"));
    assert!(!open.allows("transfer_high_value_asset"));

    let closed = schedule.decision_at(&WorldTimestamp::at_wall_time(utc(2026, 7, 5, 23, 0, 0)));
    assert!(!closed.is_active);
    assert!(!closed.allows("quote_price"));
}

#[test]
fn offline_progression_caps_elapsed_wall_time() {
    let policy = OfflineProgressionPolicy {
        allow_offline_progression: true,
        max_offline_duration_ms: 86_400_000,
        high_risk_requires_approval: true,
    };

    let report = policy.evaluate(utc(2026, 7, 1, 0, 0, 0), utc(2026, 7, 5, 0, 0, 0));

    assert_eq!(report.actual_elapsed_ms, 345_600_000);
    assert_eq!(report.applied_elapsed_ms, 86_400_000);
    assert!(report.truncated);
    assert!(report.blocked_high_risk_actions);
}

#[test]
fn replay_ledger_entries_include_wall_sim_and_tick_time() {
    let timestamp = WorldTimestamp {
        wall_time: utc(2026, 7, 5, 12, 0, 0),
        sim_time_ms: 42_000,
        tick: 42,
    };
    let mut ledger = ReplayLedger::default();

    ledger.push(ReplayLedgerEntry::new(
        "entry_01",
        timestamp.clone(),
        "guard_01",
        "patrol_intent",
        "guard_01 entered night patrol",
    ));

    let entry = ledger.by_tick(42).unwrap();
    assert_eq!(entry.timestamp, timestamp);
    assert_eq!(
        ledger
            .by_wall_time(utc(2026, 7, 5, 12, 0, 0))
            .unwrap()
            .entry_id,
        "entry_01"
    );
}

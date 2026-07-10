use agent_core::event_stream::{AgentEvent, EventReplay, EventStreamBroker};

#[test]
fn test_event_stream_persist_and_replay() {
    let dir = tempfile::tempdir().unwrap();
    let mut broker = EventStreamBroker::new(64).with_persistence(dir.path());

    let event1 = AgentEvent::AssistantMessage {
        message_id: "msg_1".into(),
        content: "Hello".into(),
        timestamp: 0,
    };
    let event2 = AgentEvent::StepCompleted {
        plan_id: "p1".into(),
        step_id: "s1".into(),
        result: "Done".into(),
        timestamp: 0,
    };
    let event3 = AgentEvent::GoalAchieved {
        goal: "Test goal".into(),
        timestamp: 0,
    };

    broker.publish(event1).ok();
    broker.publish(event2).ok();
    broker.publish(event3).ok();

    assert_eq!(broker.sequence(), 3, "Should have published 3 events");

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let jsonl_path = dir.path().join(format!("events_{}.jsonl", today));
    assert!(jsonl_path.exists(), "JSONL file should exist after publish");

    let content = std::fs::read_to_string(&jsonl_path).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 3, "JSONL should have 3 lines");

    let mut replay = EventReplay::from_file(&jsonl_path).unwrap();
    assert_eq!(replay.event_count(), 3, "Should load 3 events");

    let r1 = replay.next().unwrap();
    assert!(matches!(r1, AgentEvent::AssistantMessage { .. }));

    let r2 = replay.next().unwrap();
    assert!(matches!(r2, AgentEvent::StepCompleted { .. }));

    let r3 = replay.next().unwrap();
    assert!(matches!(r3, AgentEvent::GoalAchieved { .. }));

    assert!(replay.next().is_none(), "No more events after replay");
}

use rust_copytrader::cache::freshness::FreshnessGate;
use rust_copytrader::config::{ActivityMode, LiveModeGate};
use rust_copytrader::domain::budget::{LatencyBudget, StageBudget};
use rust_copytrader::persistence::jsonl::append_record;
use rust_copytrader::pipeline::trace_context::{Stage, TraceContext};
use std::fs;

#[test]
fn rejects_when_stage_would_breach_hard_budget() {
    let budget = LatencyBudget::new(200);
    let stage = StageBudget::new("positions", 45);

    assert!(!budget.can_schedule(170, &stage));
    assert!(budget.can_schedule(120, &stage));
}

#[test]
fn freshness_gate_rejects_stale_quotes() {
    let gate = FreshnessGate::new(10);

    assert!(gate.is_fresh(9));
    assert!(!gate.is_fresh(11));
}

#[test]
fn trace_context_records_stage_order_and_elapsed_budget() {
    let mut ctx = TraceContext::new("leader-1", "corr-1", 1_000);
    ctx.mark(Stage::ActivityObserved, 1_005);
    ctx.mark(Stage::PositionsReconciled, 1_040);
    ctx.mark(Stage::OrderSubmitted, 1_120);

    assert_eq!(ctx.total_elapsed_ms(), 120);
    assert_eq!(ctx.last_stage(), Some(Stage::OrderSubmitted));
    assert_eq!(
        ctx.stage_started_at(Stage::PositionsReconciled),
        Some(1_040)
    );
}

#[test]
fn live_listen_mode_stays_blocked_without_verified_activity_source() {
    let mut gate = LiveModeGate::for_mode(ActivityMode::LiveListen);
    gate.execution_surface_ready = true;
    gate.positions_under_budget = true;

    assert!(!gate.unlocked());
    assert_eq!(
        gate.blocked_reason().as_deref(),
        Some("activity_source_unverified")
    );

    gate.activity_source_verified = true;
    gate.activity_source_under_budget = true;
    gate.activity_capability_detected = true;
    assert!(gate.unlocked());
}

#[test]
fn jsonl_writer_appends_one_record_per_line() {
    let mut path = std::env::temp_dir();
    path.push(format!("rust-copytrader-jsonl-{}.log", std::process::id()));
    let _ = fs::remove_file(&path);

    append_record(&path, "first").unwrap();
    append_record(&path, "second").unwrap();

    let body = fs::read_to_string(&path).unwrap();
    assert_eq!(body, "first\nsecond\n");

    let _ = fs::remove_file(path);
}

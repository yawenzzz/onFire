use rust_copytrader::pipeline::trace_context::{Stage, TraceContext};
use rust_copytrader::telemetry::latency::LatencyReport;
use rust_copytrader::telemetry::metrics::RuntimeMetrics;

#[test]
fn latency_report_computes_stage_deltas_from_trace_context() {
    let mut trace = TraceContext::new("leader-1", "corr-1", 1_000);
    trace.mark(Stage::ActivityObserved, 1_000);
    trace.mark(Stage::PositionsReconciled, 1_040);
    trace.mark(Stage::MarketQuoted, 1_050);
    trace.mark(Stage::PreTradeValidated, 1_060);
    trace.mark(Stage::OrderSubmitted, 1_090);
    trace.mark(Stage::VerificationObserved, 1_110);

    let mut report = LatencyReport::default();
    report.record_trace(&trace);

    assert_eq!(report.samples(), 1);
    assert_eq!(report.stage_delta_ms(Stage::PositionsReconciled), Some(40));
    assert_eq!(report.stage_delta_ms(Stage::MarketQuoted), Some(10));
    assert_eq!(report.stage_delta_ms(Stage::OrderSubmitted), Some(30));
    assert_eq!(report.total_elapsed_max_ms(), 110);
}

#[test]
fn runtime_metrics_track_rejects_submits_and_timeouts() {
    let mut metrics = RuntimeMetrics::default();
    metrics.record_submit();
    metrics.record_submit();
    metrics.record_verified();
    metrics.record_verification_mismatch();
    metrics.record_reject("quote_stale");
    metrics.record_reject("quote_stale");
    metrics.record_reject("positions_no_net_change");
    metrics.record_verification_timeout();

    assert_eq!(metrics.submitted(), 2);
    assert_eq!(metrics.verified_total(), 1);
    assert_eq!(metrics.verification_mismatches(), 1);
    assert_eq!(metrics.rejected_total(), 3);
    assert_eq!(metrics.reject_count("quote_stale"), 2);
    assert_eq!(metrics.reject_count("positions_no_net_change"), 1);
    assert_eq!(metrics.verification_timeouts(), 1);
}

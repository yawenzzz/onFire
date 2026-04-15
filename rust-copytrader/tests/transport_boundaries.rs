use rust_copytrader::adapters::transport::{
    ActivityTransport, MarketTransport, PositionsTransport, ReplayTransportBoundary,
    VerificationTransport,
};
use rust_copytrader::adapters::verification::VerificationChannelKind;
use rust_copytrader::replay::fixture::{ReplayFixture, ReplayVerificationFrame};

#[test]
fn replay_transport_boundary_exposes_fixture_frames_without_losing_stage_data() {
    let fixture = ReplayFixture::success_buy_follow();
    let boundary = ReplayTransportBoundary::new(&fixture);

    let activity = boundary.read_activity();
    let positions = boundary.read_positions();
    let quote = boundary.read_market_quote();
    let verification = boundary.read_verification("corr-success");

    assert_eq!(boundary.transport_name(), "replay");
    assert_eq!(activity.transaction_hash, "0xtx-success");
    assert_eq!(positions.previous.current_size, 10);
    assert_eq!(positions.current.current_size, 14);
    assert_eq!(positions.reconciled_at_ms, 1_020);
    assert_eq!(quote.best_ask, 0.52);
    assert_eq!(quote.observed_at_ms, 1_028);
    assert_eq!(verification.observed_at_ms, 1_082);
    assert_eq!(
        verification
            .event
            .expect("verification event expected")
            .kind,
        VerificationChannelKind::OrderMatched
    );
}

#[test]
fn replay_transport_boundary_maps_mismatch_and_timeout_verification_frames() {
    let mut mismatch_fixture = ReplayFixture::success_buy_follow();
    mismatch_fixture.verification = ReplayVerificationFrame::Mismatch {
        observed_at_ms: 1_091,
    };
    let mismatch_boundary = ReplayTransportBoundary::new(&mismatch_fixture);
    let mismatch = mismatch_boundary.read_verification("corr-success");

    assert_eq!(mismatch.observed_at_ms, 1_091);
    assert_eq!(
        mismatch.event.expect("mismatch event expected").kind,
        VerificationChannelKind::OrderMismatch
    );

    let mut timeout_fixture = ReplayFixture::success_buy_follow();
    timeout_fixture.verification = ReplayVerificationFrame::Timeout {
        observed_at_ms: 1_120,
    };
    let timeout_boundary = ReplayTransportBoundary::new(&timeout_fixture);
    let timeout = timeout_boundary.read_verification("corr-success");

    assert_eq!(timeout.observed_at_ms, 1_120);
    assert_eq!(timeout.event, None);
}

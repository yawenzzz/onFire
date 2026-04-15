use std::cell::Cell;

use rust_copytrader::adapters::auth::AuthRuntimeState;
use rust_copytrader::adapters::order_api::{
    OrderIntent, OrderSide, PreviewRequest, PreviewResponse, PreviewSubmitGateway, SubmitRequest,
    SubmitResponse,
};
use rust_copytrader::execution::router::{ExecutionRouter, RouteOutcome, RouteRejection};

#[test]
fn auth_runtime_requires_funder_for_non_default_signature_types() {
    let auth = AuthRuntimeState::new(true, true, true, 2, false);

    assert!(!auth.submit_ready());
    assert_eq!(auth.mode_label(), "account-ready");
    assert_eq!(auth.blocked_reason(), Some("funder_required"));
}

#[test]
fn auth_runtime_reports_submit_ready_when_credentials_sdk_and_funder_align() {
    let auth = AuthRuntimeState::new(true, true, true, 2, true);

    assert!(auth.submit_ready());
    assert_eq!(auth.mode_label(), "account-auth-ready");
    assert_eq!(auth.blocked_reason(), None);
}

#[test]
fn router_rejects_before_preview_when_execution_surface_is_not_ready() {
    let mut gateway = CountingGateway::accepted();
    let auth = AuthRuntimeState::new(true, false, true, 0, false);
    let router = ExecutionRouter::new(70);

    let outcome = router.route(
        &mut gateway,
        &auth,
        OrderIntent::new("asset-9", OrderSide::Buy, 4, 0.52),
        120,
    );

    assert_eq!(
        outcome,
        RouteOutcome::Rejected(RouteRejection::ExecutionSurfaceNotReady(
            "private_key_missing".into()
        ))
    );
    assert_eq!(gateway.preview_calls.get(), 0);
    assert_eq!(gateway.submit_calls.get(), 0);
}

#[test]
fn router_rejects_before_preview_when_remaining_budget_is_exhausted() {
    let mut gateway = CountingGateway::accepted();
    let auth = AuthRuntimeState::new(true, true, true, 0, false);
    let router = ExecutionRouter::new(70);

    let outcome = router.route(
        &mut gateway,
        &auth,
        OrderIntent::new("asset-9", OrderSide::Buy, 4, 0.52),
        69,
    );

    assert_eq!(
        outcome,
        RouteOutcome::Rejected(RouteRejection::LatencyBudgetExhausted)
    );
    assert_eq!(gateway.preview_calls.get(), 0);
    assert_eq!(gateway.submit_calls.get(), 0);
}

#[test]
fn router_returns_preview_rejection_without_submitting() {
    let mut gateway = CountingGateway::preview_rejected("invalid_tick");
    let auth = AuthRuntimeState::new(true, true, true, 0, false);
    let router = ExecutionRouter::new(70);

    let outcome = router.route(
        &mut gateway,
        &auth,
        OrderIntent::new("asset-9", OrderSide::Buy, 4, 0.52),
        120,
    );

    assert_eq!(
        outcome,
        RouteOutcome::Rejected(RouteRejection::PreviewRejected("invalid_tick".into()))
    );
    assert_eq!(gateway.preview_calls.get(), 1);
    assert_eq!(gateway.submit_calls.get(), 0);
}

#[test]
fn router_returns_submit_ack_for_accepted_order() {
    let mut gateway = CountingGateway::accepted();
    let auth = AuthRuntimeState::new(true, true, true, 0, false);
    let router = ExecutionRouter::new(70);

    let outcome = router.route(
        &mut gateway,
        &auth,
        OrderIntent::new("asset-9", OrderSide::Sell, 3, 0.48),
        120,
    );

    assert_eq!(
        outcome,
        RouteOutcome::Submitted {
            submitted_at_ms: 1_060
        }
    );
    assert_eq!(gateway.preview_calls.get(), 1);
    assert_eq!(gateway.submit_calls.get(), 1);
}

struct CountingGateway {
    preview_response: PreviewResponse,
    submit_response: SubmitResponse,
    preview_calls: Cell<u64>,
    submit_calls: Cell<u64>,
}

impl CountingGateway {
    fn accepted() -> Self {
        Self {
            preview_response: PreviewResponse::accepted(),
            submit_response: SubmitResponse::accepted(1_060),
            preview_calls: Cell::new(0),
            submit_calls: Cell::new(0),
        }
    }

    fn preview_rejected(reason: &str) -> Self {
        Self {
            preview_response: PreviewResponse::rejected(reason),
            submit_response: SubmitResponse::accepted(1_060),
            preview_calls: Cell::new(0),
            submit_calls: Cell::new(0),
        }
    }
}

impl PreviewSubmitGateway for CountingGateway {
    fn preview(&mut self, _request: PreviewRequest) -> PreviewResponse {
        self.preview_calls.set(self.preview_calls.get() + 1);
        self.preview_response.clone()
    }

    fn submit(&mut self, _request: SubmitRequest) -> SubmitResponse {
        self.submit_calls.set(self.submit_calls.get() + 1);
        self.submit_response.clone()
    }
}

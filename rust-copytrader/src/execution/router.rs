use crate::adapters::auth::AuthRuntimeState;
use crate::adapters::order_api::{
    OrderIntent, PreviewRequest, PreviewSubmitGateway, SubmitRequest,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteRejection {
    ExecutionSurfaceNotReady(String),
    LatencyBudgetExhausted,
    PreviewRejected(String),
    SubmitRejected(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteOutcome {
    Rejected(RouteRejection),
    Submitted { submitted_at_ms: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionRouter {
    required_submit_budget_ms: u64,
}

impl ExecutionRouter {
    pub const fn new(required_submit_budget_ms: u64) -> Self {
        Self {
            required_submit_budget_ms,
        }
    }

    pub fn route<G: PreviewSubmitGateway>(
        &self,
        gateway: &mut G,
        auth: &AuthRuntimeState,
        intent: OrderIntent,
        remaining_budget_ms: u64,
    ) -> RouteOutcome {
        if let Some(reason) = auth.blocked_reason() {
            return RouteOutcome::Rejected(RouteRejection::ExecutionSurfaceNotReady(
                reason.to_string(),
            ));
        }
        if remaining_budget_ms < self.required_submit_budget_ms {
            return RouteOutcome::Rejected(RouteRejection::LatencyBudgetExhausted);
        }

        let preview = gateway.preview(PreviewRequest {
            asset_id: intent.asset_id.clone(),
            side: intent.side,
            size: intent.size,
            limit_price: intent.limit_price,
        });
        if !preview.accepted {
            return RouteOutcome::Rejected(RouteRejection::PreviewRejected(
                preview
                    .reason
                    .unwrap_or_else(|| "preview_rejected".to_string()),
            ));
        }

        let submit = gateway.submit(SubmitRequest {
            asset_id: intent.asset_id,
            side: intent.side,
            size: intent.size,
            limit_price: intent.limit_price,
        });
        if !submit.accepted {
            return RouteOutcome::Rejected(RouteRejection::SubmitRejected(
                submit
                    .reason
                    .unwrap_or_else(|| "submit_rejected".to_string()),
            ));
        }

        RouteOutcome::Submitted {
            submitted_at_ms: submit.submitted_at_ms.unwrap_or_default(),
        }
    }
}

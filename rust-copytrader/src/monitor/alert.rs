use super::MonitorThresholds;
use super::snapshot::{AlertView, Health, UiSnapshot};

pub fn evaluate(
    snapshot: &UiSnapshot,
    thresholds: &MonitorThresholds,
    live_mode: bool,
) -> (Health, Vec<AlertView>) {
    let mut health = Health::Ok;
    let mut alerts = Vec::new();

    if snapshot.proc.loop_lag_p95_ms > thresholds.main_loop_lag_crit_ms {
        escalate(
            &mut health,
            &mut alerts,
            Health::Crit,
            "main_loop_lag",
            format!(
                "main loop lag p95 {}ms > {}ms",
                snapshot.proc.loop_lag_p95_ms, thresholds.main_loop_lag_crit_ms
            ),
        );
    } else if snapshot.proc.loop_lag_p95_ms > thresholds.main_loop_lag_warn_ms {
        escalate(
            &mut health,
            &mut alerts,
            Health::Warn,
            "main_loop_lag",
            format!(
                "main loop lag p95 {}ms > {}ms",
                snapshot.proc.loop_lag_p95_ms, thresholds.main_loop_lag_warn_ms
            ),
        );
    }

    if snapshot.proc.monitor_dropped_total > thresholds.monitor_drop_crit_per_min {
        escalate(
            &mut health,
            &mut alerts,
            Health::Crit,
            "monitor_drop",
            format!(
                "monitor dropped total {} > {}",
                snapshot.proc.monitor_dropped_total, thresholds.monitor_drop_crit_per_min
            ),
        );
    }

    ws_alert(
        &mut health,
        &mut alerts,
        "market_ws_stale",
        snapshot.feeds.market_ws.last_msg_age_ms,
        thresholds.market_ws_age_warn_ms,
        thresholds.market_ws_age_crit_ms,
    );

    if live_mode {
        ws_alert(
            &mut health,
            &mut alerts,
            "user_ws_stale",
            snapshot.feeds.user_ws.last_msg_age_ms,
            thresholds.user_ws_age_warn_ms,
            thresholds.user_ws_age_crit_ms,
        );
    }

    if snapshot.feeds.data_api.status_429_1m > 0 {
        escalate(
            &mut health,
            &mut alerts,
            Health::Warn,
            "http_429_spike",
            format!("data api 429_1m={}", snapshot.feeds.data_api.status_429_1m),
        );
    }

    if let Some(leader) = snapshot.leaders.first() {
        if leader.activity_p95_ms > thresholds.activity_event_age_crit_ms {
            escalate(
                &mut health,
                &mut alerts,
                Health::Crit,
                "activity_event_age_high",
                format!(
                    "leader activity p95 {}ms > {}ms",
                    leader.activity_p95_ms, thresholds.activity_event_age_crit_ms
                ),
            );
        } else if leader.activity_p95_ms > thresholds.activity_event_age_warn_ms {
            escalate(
                &mut health,
                &mut alerts,
                Health::Warn,
                "activity_event_age_high",
                format!(
                    "leader activity p95 {}ms > {}ms",
                    leader.activity_p95_ms, thresholds.activity_event_age_warn_ms
                ),
            );
        }

        if leader.reconcile_p95_ms > thresholds.reconcile_crit_ms {
            escalate(
                &mut health,
                &mut alerts,
                Health::Crit,
                "positions_slow",
                format!(
                    "leader reconcile p95 {}ms > {}ms",
                    leader.reconcile_p95_ms, thresholds.reconcile_crit_ms
                ),
            );
        } else if leader.reconcile_p95_ms > thresholds.reconcile_warn_ms {
            escalate(
                &mut health,
                &mut alerts,
                Health::Warn,
                "positions_slow",
                format!(
                    "leader reconcile p95 {}ms > {}ms",
                    leader.reconcile_p95_ms, thresholds.reconcile_warn_ms
                ),
            );
        }
    }

    if let Some(book) = snapshot.books.first() {
        if book.age_ms > thresholds.book_age_crit_ms {
            escalate(
                &mut health,
                &mut alerts,
                Health::Crit,
                "book_stale",
                format!(
                    "book age {}ms > {}ms",
                    book.age_ms, thresholds.book_age_crit_ms
                ),
            );
        } else if book.age_ms > thresholds.book_age_warn_ms {
            escalate(
                &mut health,
                &mut alerts,
                Health::Warn,
                "book_stale",
                format!(
                    "book age {}ms > {}ms",
                    book.age_ms, thresholds.book_age_warn_ms
                ),
            );
        }
    }

    if snapshot.exec.copy_gap_p95_bps > thresholds.copy_gap_crit_bps {
        escalate(
            &mut health,
            &mut alerts,
            Health::Crit,
            "copy_gap_wide",
            format!(
                "copy gap p95 {}bp > {}bp",
                snapshot.exec.copy_gap_p95_bps, thresholds.copy_gap_crit_bps
            ),
        );
    } else if snapshot.exec.copy_gap_p95_bps > thresholds.copy_gap_warn_bps {
        escalate(
            &mut health,
            &mut alerts,
            Health::Warn,
            "copy_gap_wide",
            format!(
                "copy gap p95 {}bp > {}bp",
                snapshot.exec.copy_gap_p95_bps, thresholds.copy_gap_warn_bps
            ),
        );
    }

    if snapshot.risk.rmse_1m_bps > thresholds.track_rmse_crit_bps as u16 {
        escalate(
            &mut health,
            &mut alerts,
            Health::Crit,
            "tracking_error_high",
            format!(
                "tracking rmse 1m {}bp > {}bp",
                snapshot.risk.rmse_1m_bps, thresholds.track_rmse_crit_bps
            ),
        );
    } else if snapshot.risk.rmse_1m_bps > thresholds.track_rmse_warn_bps as u16 {
        escalate(
            &mut health,
            &mut alerts,
            Health::Warn,
            "tracking_error_high",
            format!(
                "tracking rmse 1m {}bp > {}bp",
                snapshot.risk.rmse_1m_bps, thresholds.track_rmse_warn_bps
            ),
        );
    }

    if snapshot.risk.tail_24h_usdc > 0 {
        escalate(
            &mut health,
            &mut alerts,
            Health::Warn,
            "tail_exposure_present",
            format!("tail <24h exposure {}", snapshot.risk.tail_24h_usdc),
        );
    }

    if snapshot.risk.neg_risk_usdc > 0 {
        escalate(
            &mut health,
            &mut alerts,
            Health::Warn,
            "neg_risk_exposure_present",
            format!("neg risk exposure {}", snapshot.risk.neg_risk_usdc),
        );
    }

    if snapshot.risk.follow_ratio_bps > 0 && snapshot.risk.follow_ratio_bps < 7_000 {
        escalate(
            &mut health,
            &mut alerts,
            Health::Warn,
            "follow_ratio_low",
            format!("follow ratio {}bp < 7000bp", snapshot.risk.follow_ratio_bps),
        );
    }

    (health, alerts)
}

fn ws_alert(
    health: &mut Health,
    alerts: &mut Vec<AlertView>,
    key: &str,
    age_ms: u64,
    warn_ms: u64,
    crit_ms: u64,
) {
    if age_ms > crit_ms {
        escalate(
            health,
            alerts,
            Health::Crit,
            key,
            format!("{} age {}ms > {}ms", key, age_ms, crit_ms),
        );
    } else if age_ms > warn_ms {
        escalate(
            health,
            alerts,
            Health::Warn,
            key,
            format!("{} age {}ms > {}ms", key, age_ms, warn_ms),
        );
    }
}

fn escalate(
    health: &mut Health,
    alerts: &mut Vec<AlertView>,
    level: Health,
    key: &str,
    message: String,
) {
    *health = (*health).max(level);
    alerts.push(AlertView {
        level,
        key: key.to_string(),
        message,
    });
}

#[cfg(test)]
mod tests {
    use super::evaluate;
    use crate::monitor::MonitorThresholds;
    use crate::monitor::snapshot::{
        BookViewUi, FeedView, Health, LeaderView, PositionTargetingView, ProcView, RiskView,
        SelectedLeaderView, TrackedActivityView, UiSnapshot,
    };

    #[test]
    fn evaluate_flags_activity_reconcile_and_book_staleness() {
        let snapshot = UiSnapshot {
            proc: ProcView::default(),
            feeds: FeedView::default(),
            selected_leader: SelectedLeaderView::default(),
            tracked_activity: TrackedActivityView::default(),
            leaders: vec![LeaderView {
                activity_p95_ms: 12_000,
                reconcile_p95_ms: 3_500,
                ..LeaderView::default()
            }],
            books: vec![BookViewUi {
                age_ms: 3_100,
                ..BookViewUi::default()
            }],
            position_targeting: PositionTargetingView::default(),
            risk: RiskView::default(),
            ..UiSnapshot::default()
        };

        let (health, alerts) = evaluate(&snapshot, &MonitorThresholds::default(), false);
        assert_eq!(health, Health::Crit);
        let keys = alerts
            .into_iter()
            .map(|alert| alert.key)
            .collect::<Vec<_>>();
        assert!(keys.contains(&"activity_event_age_high".to_string()));
        assert!(keys.contains(&"positions_slow".to_string()));
        assert!(keys.contains(&"book_stale".to_string()));
    }
}

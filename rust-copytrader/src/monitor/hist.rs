use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollingHistogram {
    window_ms: u64,
    samples: VecDeque<(u64, u64)>,
}

impl RollingHistogram {
    pub fn new(window_ms: u64) -> Self {
        Self {
            window_ms: window_ms.max(1),
            samples: VecDeque::new(),
        }
    }

    fn prune(&mut self, now_ms: u64) {
        while let Some((ts, _)) = self.samples.front().copied() {
            if now_ms.saturating_sub(ts) <= self.window_ms {
                break;
            }
            self.samples.pop_front();
        }
    }

    pub fn record(&mut self, now_ms: u64, value: u64) {
        self.prune(now_ms);
        self.samples.push_back((now_ms, value));
    }

    pub fn p50(&mut self, now_ms: u64) -> u64 {
        self.quantile(now_ms, 50)
    }

    pub fn p95(&mut self, now_ms: u64) -> u64 {
        self.quantile(now_ms, 95)
    }

    pub fn quantile(&mut self, now_ms: u64, percentile: u64) -> u64 {
        self.prune(now_ms);
        if self.samples.is_empty() {
            return 0;
        }
        let mut values = self
            .samples
            .iter()
            .map(|(_, value)| *value)
            .collect::<Vec<_>>();
        values.sort_unstable();
        let percentile = percentile.clamp(0, 100);
        let index = if values.len() == 1 {
            0
        } else {
            ((values.len() - 1) * percentile as usize) / 100
        };
        values[index]
    }

    pub fn last(&mut self, now_ms: u64) -> Option<u64> {
        self.prune(now_ms);
        self.samples.back().map(|(_, value)| *value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollingRms {
    window_ms: u64,
    samples: VecDeque<(u64, u64)>,
}

impl RollingRms {
    pub fn new(window_ms: u64) -> Self {
        Self {
            window_ms: window_ms.max(1),
            samples: VecDeque::new(),
        }
    }

    fn prune(&mut self, now_ms: u64) {
        while let Some((ts, _)) = self.samples.front().copied() {
            if now_ms.saturating_sub(ts) <= self.window_ms {
                break;
            }
            self.samples.pop_front();
        }
    }

    pub fn record(&mut self, now_ms: u64, value: u64) {
        self.prune(now_ms);
        self.samples.push_back((now_ms, value));
    }

    pub fn rmse(&mut self, now_ms: u64) -> u64 {
        self.prune(now_ms);
        if self.samples.is_empty() {
            return 0;
        }
        let sum_sq = self.samples.iter().fold(0f64, |acc, (_, value)| {
            acc + (*value as f64) * (*value as f64)
        });
        (sum_sq / self.samples.len() as f64).sqrt().round() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::{RollingHistogram, RollingRms};

    #[test]
    fn histogram_returns_percentiles() {
        let mut hist = RollingHistogram::new(60_000);
        for value in [10, 20, 30, 40, 50] {
            hist.record(1_000, value);
        }
        assert_eq!(hist.p50(1_000), 30);
        assert_eq!(hist.p95(1_000), 40);
    }

    #[test]
    fn rolling_rms_uses_recent_samples() {
        let mut rms = RollingRms::new(2_000);
        rms.record(0, 30);
        rms.record(1_000, 40);
        assert_eq!(rms.rmse(1_500), 35);
        assert_eq!(rms.rmse(5_000), 0);
    }
}

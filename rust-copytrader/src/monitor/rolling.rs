#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollingCounter {
    slot_ms: u64,
    slots: Vec<u64>,
    head: usize,
    last_roll_ms: u64,
    sum: u64,
}

impl RollingCounter {
    pub fn new(slot_ms: u64, slot_count: usize, now_ms: u64) -> Self {
        Self {
            slot_ms: slot_ms.max(1),
            slots: vec![0; slot_count.max(1)],
            head: 0,
            last_roll_ms: now_ms,
            sum: 0,
        }
    }

    fn rotate(&mut self, now_ms: u64) {
        if now_ms <= self.last_roll_ms {
            return;
        }
        let elapsed = now_ms - self.last_roll_ms;
        let steps = (elapsed / self.slot_ms) as usize;
        if steps == 0 {
            return;
        }
        if steps >= self.slots.len() {
            self.slots.fill(0);
            self.head = 0;
            self.sum = 0;
            self.last_roll_ms = now_ms;
            return;
        }
        for _ in 0..steps {
            self.head = (self.head + 1) % self.slots.len();
            self.sum = self.sum.saturating_sub(self.slots[self.head]);
            self.slots[self.head] = 0;
        }
        self.last_roll_ms = self
            .last_roll_ms
            .saturating_add((steps as u64).saturating_mul(self.slot_ms));
    }

    pub fn incr(&mut self, now_ms: u64, value: u64) {
        self.rotate(now_ms);
        self.slots[self.head] = self.slots[self.head].saturating_add(value);
        self.sum = self.sum.saturating_add(value);
    }

    pub fn sum(&mut self, now_ms: u64) -> u64 {
        self.rotate(now_ms);
        self.sum
    }
}

#[cfg(test)]
mod tests {
    use super::RollingCounter;

    #[test]
    fn rolling_counter_rotates_and_expires_old_slots() {
        let mut counter = RollingCounter::new(1_000, 3, 0);
        counter.incr(0, 2);
        counter.incr(500, 1);
        assert_eq!(counter.sum(999), 3);
        counter.incr(1_500, 4);
        assert_eq!(counter.sum(1_500), 7);
        assert_eq!(counter.sum(4_100), 0);
    }
}

use std::time::{Duration, Instant, SystemTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Pending,
    Up,
    Down,
}

#[derive(Debug, Clone)]
pub struct TargetState {
    pub health: Health,
    pub latency: Option<Duration>,
    pub detail: String,
    pub checked_at: Option<SystemTime>,
    pub down_since: Option<Instant>,
    pub consecutive_failures: u32,
    pub alarmed: bool,
}

impl Default for TargetState {
    fn default() -> Self {
        Self {
            health: Health::Pending,
            latency: None,
            detail: String::new(),
            checked_at: None,
            down_since: None,
            consecutive_failures: 0,
            alarmed: false,
        }
    }
}

impl TargetState {
    pub fn record(&mut self, result: Result<String, String>, latency: Duration, now: Instant) {
        match result {
            Ok(detail) => {
                self.health = Health::Up;
                self.detail = detail;
                self.down_since = None;
                self.consecutive_failures = 0;
                self.alarmed = false;
            }
            Err(detail) => {
                self.health = Health::Down;
                self.detail = detail;
                self.down_since.get_or_insert(now);
                self.consecutive_failures = self.consecutive_failures.saturating_add(1);
            }
        }
        self.latency = Some(latency);
        self.checked_at = Some(SystemTime::now());
    }

    pub fn alarm_due(&mut self, threshold: u32, enabled: bool) -> bool {
        if enabled
            && self.health == Health::Down
            && !self.alarmed
            && self.consecutive_failures >= threshold
        {
            self.alarmed = true;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alarm_triggers_once_at_threshold_and_rearms_after_recovery() {
        let mut state = TargetState::default();
        let now = Instant::now();
        let latency = Duration::from_millis(5);
        assert!(!state.alarm_due(3, true));
        for i in 0..2 {
            state.record(Err("down".into()), latency, now + Duration::from_secs(i));
            assert!(!state.alarm_due(3, true));
            assert_eq!(state.down_since, Some(now));
        }
        state.record(Err("down".into()), latency, now + Duration::from_secs(2));
        assert!(!state.alarm_due(3, false));
        assert!(!state.alarmed);
        assert!(state.alarm_due(3, true));
        assert!(!state.alarm_due(3, true));
        state.record(Err("down".into()), latency, now + Duration::from_secs(3));
        assert!(!state.alarm_due(3, true));
        state.record(Ok("up".into()), latency, now + Duration::from_secs(4));
        assert_eq!(state.health, Health::Up);
        assert_eq!(state.consecutive_failures, 0);
        assert_eq!(state.down_since, None);
        assert!(!state.alarmed);
        state.record(Err("down".into()), latency, now + Duration::from_secs(5));
        assert_eq!(state.down_since, Some(now + Duration::from_secs(5)));
        assert!(state.alarm_due(1, true));
    }

    #[test]
    fn failure_count_saturates_and_success_resets_it() {
        let mut state = TargetState {
            consecutive_failures: u32::MAX,
            ..TargetState::default()
        };
        let now = Instant::now();
        for _ in 0..41 {
            state.record(Err("down".into()), Duration::ZERO, now);
        }
        assert_eq!(state.consecutive_failures, u32::MAX);
        state.record(Ok("up".into()), Duration::from_millis(4), now);
        assert_eq!(state.consecutive_failures, 0);
        assert_eq!(state.latency, Some(Duration::from_millis(4)));
        assert!(state.checked_at.is_some());
    }
}

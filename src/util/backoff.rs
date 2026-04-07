use std::time::Duration;

/// Exponential backoff iterator.
///
/// Each call to [`next_delay`](ExponentialBackoff::next_delay) returns the
/// duration to sleep before the next attempt, multiplying the previous delay by
/// `multiplier` and capping at `max`.  Returns `None` once `max_attempts` have
/// been exhausted.
pub struct ExponentialBackoff {
    /// Kept for [`reset`](Self::reset).
    #[allow(dead_code)]
    initial: Duration,
    current: Duration,
    multiplier: f64,
    max: Duration,
    attempts: u32,
    max_attempts: u32,
}

impl ExponentialBackoff {
    /// Create a new backoff with the given parameters.
    ///
    /// * `initial`      -- first delay
    /// * `multiplier`   -- factor applied after each attempt (e.g. 1.5)
    /// * `max`          -- ceiling for any single delay
    /// * `max_attempts` -- total attempts before giving up
    pub fn new(initial: Duration, multiplier: f64, max: Duration, max_attempts: u32) -> Self {
        Self {
            initial,
            current: initial,
            multiplier,
            max,
            attempts: 0,
            max_attempts,
        }
    }

    /// Returns the next delay to sleep, or `None` if `max_attempts` reached.
    pub fn next_delay(&mut self) -> Option<Duration> {
        if self.attempts >= self.max_attempts {
            return None;
        }
        let delay = self.current;
        self.attempts += 1;

        // Advance for next call.
        let next_secs = self.current.as_secs_f64() * self.multiplier;
        self.current = Duration::from_secs_f64(next_secs.min(self.max.as_secs_f64()));

        Some(delay)
    }

    /// Reset the backoff to its initial state.
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.current = self.initial;
        self.attempts = 0;
    }

    /// Number of attempts consumed so far.
    pub fn attempts(&self) -> u32 {
        self.attempts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_progression() {
        let mut b = ExponentialBackoff::new(
            Duration::from_millis(100),
            2.0,
            Duration::from_millis(500),
            5,
        );

        // 100, 200, 400, 500 (capped), 500 (capped)
        assert_eq!(b.next_delay(), Some(Duration::from_millis(100)));
        assert_eq!(b.next_delay(), Some(Duration::from_millis(200)));
        assert_eq!(b.next_delay(), Some(Duration::from_millis(400)));
        // Next would be 800 but capped at 500
        let d = b.next_delay().unwrap();
        assert!(d <= Duration::from_millis(500));
        let d = b.next_delay().unwrap();
        assert!(d <= Duration::from_millis(500));
        // Exhausted
        assert_eq!(b.next_delay(), None);
        assert_eq!(b.attempts(), 5);
    }

    #[test]
    fn reset_restarts() {
        let mut b = ExponentialBackoff::new(
            Duration::from_millis(10),
            2.0,
            Duration::from_secs(1),
            2,
        );
        assert!(b.next_delay().is_some());
        assert!(b.next_delay().is_some());
        assert!(b.next_delay().is_none());

        b.reset();
        assert_eq!(b.attempts(), 0);
        assert_eq!(b.next_delay(), Some(Duration::from_millis(10)));
    }

    #[test]
    fn zero_max_attempts_returns_none_immediately() {
        let mut b = ExponentialBackoff::new(
            Duration::from_millis(100),
            2.0,
            Duration::from_secs(10),
            0,
        );
        assert_eq!(b.next_delay(), None);
        assert_eq!(b.attempts(), 0);
    }

    #[test]
    fn single_attempt() {
        let mut b = ExponentialBackoff::new(
            Duration::from_millis(50),
            2.0,
            Duration::from_secs(10),
            1,
        );
        assert_eq!(b.next_delay(), Some(Duration::from_millis(50)));
        assert_eq!(b.next_delay(), None);
        assert_eq!(b.attempts(), 1);
    }

    #[test]
    fn multiplier_one_stays_constant() {
        let mut b = ExponentialBackoff::new(
            Duration::from_millis(200),
            1.0,
            Duration::from_secs(10),
            3,
        );
        assert_eq!(b.next_delay(), Some(Duration::from_millis(200)));
        assert_eq!(b.next_delay(), Some(Duration::from_millis(200)));
        assert_eq!(b.next_delay(), Some(Duration::from_millis(200)));
        assert_eq!(b.next_delay(), None);
    }

    #[test]
    fn initial_exceeds_max_caps_immediately() {
        let mut b = ExponentialBackoff::new(
            Duration::from_secs(60),
            2.0,
            Duration::from_secs(10),
            3,
        );
        let first = b.next_delay().unwrap();
        assert_eq!(first, Duration::from_secs(60));
        let second = b.next_delay().unwrap();
        assert!(
            second <= Duration::from_secs(10),
            "after first delay the cap should apply: got {second:?}"
        );
    }

    #[test]
    fn reset_after_partial_use() {
        let mut b = ExponentialBackoff::new(
            Duration::from_millis(10),
            2.0,
            Duration::from_secs(1),
            5,
        );
        assert!(b.next_delay().is_some());
        assert!(b.next_delay().is_some());
        assert_eq!(b.attempts(), 2);

        b.reset();
        assert_eq!(b.attempts(), 0);
        assert_eq!(b.next_delay(), Some(Duration::from_millis(10)));
        assert_eq!(b.attempts(), 1);
    }

    #[test]
    fn fractional_multiplier() {
        let mut b = ExponentialBackoff::new(
            Duration::from_millis(1000),
            1.5,
            Duration::from_secs(10),
            3,
        );
        assert_eq!(b.next_delay(), Some(Duration::from_millis(1000)));
        assert_eq!(b.next_delay(), Some(Duration::from_millis(1500)));
        assert_eq!(b.next_delay(), Some(Duration::from_millis(2250)));
        assert_eq!(b.next_delay(), None);
    }
}

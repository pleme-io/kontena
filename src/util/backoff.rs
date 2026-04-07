use std::fmt;
use std::time::Duration;

/// Exponential backoff iterator.
///
/// Each call to [`next_delay`](ExponentialBackoff::next_delay) (or
/// [`Iterator::next`]) returns the duration to sleep before the next attempt,
/// multiplying the previous delay by `multiplier` and capping at `max`.
/// Returns `None` once `max_attempts` have been exhausted.
///
/// Implements [`Iterator`] so it can be used in `for` loops and adaptor chains.
#[must_use]
pub(crate) struct ExponentialBackoff {
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
    pub(crate) fn new(initial: Duration, multiplier: f64, max: Duration, max_attempts: u32) -> Self {
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
    pub(crate) fn next_delay(&mut self) -> Option<Duration> {
        if self.attempts >= self.max_attempts {
            return None;
        }
        let delay = self.current;
        self.attempts += 1;

        let next_secs = self.current.as_secs_f64() * self.multiplier;
        self.current = Duration::from_secs_f64(next_secs.min(self.max.as_secs_f64()));

        Some(delay)
    }

    /// Reset the backoff to its initial state.
    #[allow(dead_code)]
    pub(crate) fn reset(&mut self) {
        self.current = self.initial;
        self.attempts = 0;
    }

    /// Number of attempts consumed so far.
    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn attempts(&self) -> u32 {
        self.attempts
    }
}

impl fmt::Display for ExponentialBackoff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExponentialBackoff(initial={:?}, ×{}, max={:?}, {}/{})",
            self.initial, self.multiplier, self.max, self.attempts, self.max_attempts,
        )
    }
}

/// Sensible defaults: 1 s initial, 2× multiplier, 30 s cap, 10 attempts.
impl Default for ExponentialBackoff {
    fn default() -> Self {
        Self::new(
            Duration::from_secs(1),
            2.0,
            Duration::from_secs(30),
            10,
        )
    }
}

impl Iterator for ExponentialBackoff {
    type Item = Duration;

    fn next(&mut self) -> Option<Duration> {
        self.next_delay()
    }
}

/// Builder for [`ExponentialBackoff`] with ergonomic defaults.
///
/// All fields default to the same values as [`ExponentialBackoff::default`].
#[must_use]
#[allow(dead_code)]
pub(crate) struct BackoffBuilder {
    initial: Duration,
    multiplier: f64,
    max: Duration,
    max_attempts: u32,
}

impl Default for BackoffBuilder {
    fn default() -> Self {
        Self {
            initial: Duration::from_secs(1),
            multiplier: 2.0,
            max: Duration::from_secs(30),
            max_attempts: 10,
        }
    }
}

#[allow(dead_code)]
impl BackoffBuilder {
    pub(crate) fn initial(mut self, d: Duration) -> Self {
        self.initial = d;
        self
    }

    pub(crate) fn multiplier(mut self, m: f64) -> Self {
        self.multiplier = m;
        self
    }

    pub(crate) fn max(mut self, d: Duration) -> Self {
        self.max = d;
        self
    }

    pub(crate) fn max_attempts(mut self, n: u32) -> Self {
        self.max_attempts = n;
        self
    }

    pub(crate) fn build(self) -> ExponentialBackoff {
        ExponentialBackoff::new(self.initial, self.multiplier, self.max, self.max_attempts)
    }
}

impl ExponentialBackoff {
    /// Returns a [`BackoffBuilder`] for fluent construction.
    #[allow(dead_code)]
    pub(crate) fn builder() -> BackoffBuilder {
        BackoffBuilder::default()
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

        assert_eq!(b.next_delay(), Some(Duration::from_millis(100)));
        assert_eq!(b.next_delay(), Some(Duration::from_millis(200)));
        assert_eq!(b.next_delay(), Some(Duration::from_millis(400)));
        let d = b.next_delay().unwrap();
        assert!(d <= Duration::from_millis(500));
        let d = b.next_delay().unwrap();
        assert!(d <= Duration::from_millis(500));
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

    #[test]
    fn default_produces_ten_delays() {
        let b = ExponentialBackoff::default();
        assert_eq!(b.attempts(), 0);
        let delays: Vec<_> = b.collect();
        assert_eq!(delays.len(), 10);
        assert_eq!(delays[0], Duration::from_secs(1));
        assert!(delays.last().unwrap() <= &Duration::from_secs(30));
    }

    #[test]
    fn iterator_matches_next_delay() {
        let mut manual = ExponentialBackoff::new(
            Duration::from_millis(100),
            2.0,
            Duration::from_millis(500),
            3,
        );
        let iter_b = ExponentialBackoff::new(
            Duration::from_millis(100),
            2.0,
            Duration::from_millis(500),
            3,
        );
        let iter_vals: Vec<_> = iter_b.collect();
        for d in &iter_vals {
            assert_eq!(manual.next_delay().as_ref(), Some(d));
        }
        assert_eq!(manual.next_delay(), None);
    }

    #[test]
    fn display_includes_state() {
        let b = ExponentialBackoff::new(
            Duration::from_millis(100),
            2.0,
            Duration::from_secs(5),
            3,
        );
        let s = b.to_string();
        assert!(s.contains("100ms"), "should show initial: {s}");
        assert!(s.contains("×2"), "should show multiplier: {s}");
        assert!(s.contains("0/3"), "should show attempts/max: {s}");
    }

    #[test]
    fn builder_defaults_match_default() {
        let from_default: Vec<_> = ExponentialBackoff::default().collect();
        let from_builder: Vec<_> = ExponentialBackoff::builder().build().collect();
        assert_eq!(from_default, from_builder);
    }

    #[test]
    fn builder_overrides() {
        let b = ExponentialBackoff::builder()
            .initial(Duration::from_millis(50))
            .multiplier(3.0)
            .max(Duration::from_millis(200))
            .max_attempts(2)
            .build();
        let delays: Vec<_> = b.collect();
        assert_eq!(delays.len(), 2);
        assert_eq!(delays[0], Duration::from_millis(50));
        assert_eq!(delays[1], Duration::from_millis(150));
    }
}

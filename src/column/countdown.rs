//! Countdown timer: counts down to zero and then rings once.

use crate::clock::{MS_PER_SECOND, format_hms};

/// What the display should look like (the theme picks the colors).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CountdownState {
    /// Never started since it was set.
    #[default]
    Idle,
    Running,
    /// Stopped by the user, or finished.
    Paused,
}

/// Same idea as the stopwatch: while running we remember when it started
/// and compute the remaining time from `now`.
#[derive(Debug, Default)]
pub struct Countdown {
    running: bool,
    /// When the current run started (valid while running).
    started_at_ms: i64,
    /// Time left when the current run started, or time left while paused.
    remaining_ms: i64,
    state: CountdownState,
}

impl Countdown {
    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn state(&self) -> CountdownState {
        self.state
    }

    pub fn remaining_ms(&self, now: i64) -> i64 {
        if self.running {
            self.remaining_ms - (now - self.started_at_ms)
        } else {
            self.remaining_ms
        }
    }

    pub fn text(&self, now: i64) -> String {
        format_hms(self.remaining_ms(now))
    }

    /// Can only start when it's paused and there's at least one second left.
    pub fn can_play(&self) -> bool {
        !self.running && self.remaining_ms >= MS_PER_SECOND
    }

    pub fn play(&mut self, now: i64) {
        if self.can_play() {
            self.running = true;
            self.started_at_ms = now;
            self.state = CountdownState::Running;
        }
    }

    pub fn stop(&mut self, now: i64) {
        if self.running {
            self.remaining_ms = self.remaining_ms(now).max(0);
            self.running = false;
            self.state = CountdownState::Paused;
        }
    }

    /// The "set" button: pause and load a new duration (doesn't start it).
    pub fn set(&mut self, duration_ms: i64, now: i64) {
        self.stop(now);
        self.remaining_ms = duration_ms;
    }

    /// Call regularly. Returns `true` exactly once, when the countdown
    /// reaches 00:00:00 (the caller then plays the sound).
    pub fn check_finished(&mut self, now: i64) -> bool {
        if self.running && self.remaining_ms(now) < MS_PER_SECOND {
            self.stop(now);
            self.remaining_ms = 0;
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
    fn counts_down_and_finishes_once() {
        let mut c = Countdown::default();
        c.set(2_000, 0);
        c.play(0);
        assert!(!c.check_finished(500));
        assert_eq!(c.text(500), "00:00:01");
        assert!(c.check_finished(1_500));
        assert!(!c.is_running());
        assert!(!c.check_finished(2_000), "only reports the end once");
    }

    #[test]
    fn does_not_start_empty() {
        let mut c = Countdown::default();
        c.play(0);
        assert!(!c.is_running());
    }
}

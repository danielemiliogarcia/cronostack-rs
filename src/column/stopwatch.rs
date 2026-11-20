//! The chronometer itself: counts up, can be paused, resumed and reset.

use crate::clock::{ZERO_TIME, format_hms};

/// What the display should look like (the theme picks the colors).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopwatchState {
    Running,
    Paused,
    /// Paused at 00:00:00.
    Zero,
}

/// How it works: while running we only remember *when it would have shown
/// zero* (`zero_at_ms`); the elapsed time is then simply `now - zero_at_ms`.
/// When paused we store the elapsed time in `paused_elapsed_ms` instead.
#[derive(Debug, Default)]
pub struct Stopwatch {
    running: bool,
    /// Timestamp at which the stopwatch would read 00:00:00 (valid while running).
    zero_at_ms: i64,
    /// Elapsed time while paused (or the starting point for the next play).
    paused_elapsed_ms: i64,
}

impl Stopwatch {
    /// A paused stopwatch that already shows `elapsed_ms` (used when loading).
    pub fn paused_at(elapsed_ms: i64) -> Self {
        Self {
            running: false,
            zero_at_ms: 0,
            paused_elapsed_ms: elapsed_ms.max(0),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Starts (or resumes) counting. Returns `true` if it was actually
    /// started by this call, `false` if it was already running.
    pub fn play(&mut self, now: i64) -> bool {
        if self.running {
            return false;
        }
        self.zero_at_ms = now - self.paused_elapsed_ms;
        self.running = true;
        true
    }

    /// Pauses, keeping the elapsed time.
    pub fn stop(&mut self, now: i64) {
        if self.running {
            self.paused_elapsed_ms = now - self.zero_at_ms;
            self.running = false;
        }
    }

    /// Back to 00:00:00, paused.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// The "set" button: jump to `elapsed_ms` and keep counting from there.
    /// Returns `true` if this started the stopwatch.
    pub fn set_and_play(&mut self, elapsed_ms: i64, now: i64) -> bool {
        self.stop(now);
        self.paused_elapsed_ms = elapsed_ms;
        self.play(now)
    }

    pub fn elapsed_ms(&self, now: i64) -> i64 {
        if self.running {
            now - self.zero_at_ms
        } else {
            self.paused_elapsed_ms
        }
    }

    pub fn text(&self, now: i64) -> String {
        format_hms(self.elapsed_ms(now))
    }

    pub fn state(&self, now: i64) -> StopwatchState {
        if self.running {
            StopwatchState::Running
        } else if self.text(now) == ZERO_TIME {
            StopwatchState::Zero
        } else {
            StopwatchState::Paused
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_stop_resume_reset() {
        let mut s = Stopwatch::default();
        assert!(s.play(1_000));
        assert!(!s.play(1_500), "already running");
        assert_eq!(s.text(6_000), "00:00:05");

        s.stop(6_000);
        assert_eq!(s.text(100_000), "00:00:05", "paused time doesn't move");

        s.play(100_000);
        assert_eq!(s.text(102_000), "00:00:07", "resumes where it was");

        assert_eq!(s.state(102_000), StopwatchState::Running);
        s.reset();
        assert_eq!(s.text(200_000), ZERO_TIME);
        assert_eq!(s.state(200_000), StopwatchState::Zero);
    }

    #[test]
    fn set_jumps_and_starts() {
        let mut s = Stopwatch::default();
        assert!(s.set_and_play(90 * 60_000, 0));
        assert_eq!(s.text(1_000), "01:30:01");
    }
}

//! Daily alarm: rings when the local clock reaches a given time of day.

use crate::clock::{MS_PER_DAY, ZERO_TIME, format_hms};
use crate::sound::AlarmPlayer;

#[derive(Default)]
pub struct Alarm {
    /// Alarm time as `HH:MM:SS`, compared directly with the local clock text.
    time: String,
    enabled: bool,
    /// True while we're inside the alarm's second and already rang, so it
    /// doesn't start again on every frame of that same second.
    rang_this_second: bool,
    /// Plays the beeps in a background thread.
    player: AlarmPlayer,
}

impl Alarm {
    pub fn new() -> Self {
        Self {
            time: ZERO_TIME.to_owned(),
            ..Default::default()
        }
    }

    pub fn time_text(&self) -> &str {
        &self.time
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Sets the time of day. The alarm is a clock time, so it wraps at 24 h
    /// ("25:05" becomes 01:05:00).
    pub fn set_time(&mut self, time_of_day_ms: i64) {
        self.time = format_hms(time_of_day_ms.rem_euclid(MS_PER_DAY));
        self.rang_this_second = false;
    }

    /// The "on" checkbox. Turning it off also silences a ringing alarm.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.player.stop();
        }
    }

    /// Call regularly with the current local time (`HH:MM:SS`).
    pub fn check(&mut self, local_time: &str) {
        if self.enabled && local_time == self.time {
            if !self.rang_this_second {
                self.rang_this_second = true;
                self.player.start();
            }
        } else {
            self.rang_this_second = false;
        }
    }

    /// Silences the alarm (used when the chronometer is removed).
    pub fn silence(&self) {
        self.player.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_wraps_at_24h() {
        let mut a = Alarm::new();
        a.set_time(25 * 3_600_000 + 5 * 60_000);
        assert_eq!(a.time_text(), "01:05:00");
    }
}

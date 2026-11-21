//! Time helpers shared by the whole app.
//!
//! All timing is done with plain millisecond Unix timestamps passed in as
//! `now`. That keeps the timer logic pure and easy to test: tests just pass
//! made-up `now` values instead of waiting.

use chrono::Local;

pub const MS_PER_SECOND: i64 = 1_000;
pub const MS_PER_MINUTE: i64 = 60 * MS_PER_SECOND;
pub const MS_PER_HOUR: i64 = 60 * MS_PER_MINUTE;
pub const MS_PER_DAY: i64 = 24 * MS_PER_HOUR;

/// Text shown by an empty chronometer/countdown/alarm.
pub const ZERO_TIME: &str = "00:00:00";

/// Current wall-clock time in milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Formats a duration as `HH:MM:SS`.
///
/// Negative durations show as `00:00:00`. Hours are not wrapped at 24, so a
/// stopwatch that ran for 25 hours shows `25:00:00`.
pub fn format_hms(ms: i64) -> String {
    let total_secs = ms.max(0) / MS_PER_SECOND;
    let hours = total_secs / 3600;
    let minutes = (total_secs / 60) % 60;
    let seconds = total_secs % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

/// Local time of day as `HH:MM:SS` (24 h), used by the clock and the alarm.
pub fn local_time_hms() -> String {
    Local::now().format("%H:%M:%S").to_string()
}

/// Today's date as `dd/MM/yyyy`, used by the "Copy" button.
pub fn today_dmy() -> String {
    Local::now().format("%d/%m/%Y").to_string()
}

/// Converts the text of an "hours" and a "minutes" field into milliseconds.
///
/// Anything that isn't a number counts as 0, so an empty hours field with
/// "30" minutes means 30 minutes.
pub fn hours_minutes_to_ms(hours: &str, minutes: &str) -> i64 {
    let parse = |s: &str| s.trim().parse::<i64>().unwrap_or(0);
    parse(hours) * MS_PER_HOUR + parse(minutes) * MS_PER_MINUTE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_durations() {
        assert_eq!(format_hms(0), "00:00:00");
        assert_eq!(format_hms(27_000), "00:00:27");
        assert_eq!(format_hms(3_723_999), "01:02:03");
        assert_eq!(format_hms(-5), "00:00:00");
        assert_eq!(format_hms(25 * MS_PER_HOUR), "25:00:00");
    }

    #[test]
    fn parses_hours_and_minutes() {
        assert_eq!(hours_minutes_to_ms("1", "30"), 90 * MS_PER_MINUTE);
        assert_eq!(hours_minutes_to_ms("", "5"), 5 * MS_PER_MINUTE);
        assert_eq!(hours_minutes_to_ms("x", ""), 0);
    }
}

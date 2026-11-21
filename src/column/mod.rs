//! One column of the window: a stopwatch plus optional extras.
//!
//! Each section can be shown or hidden from the column's settings:
//!
//! | section     | logic in        |
//! |-------------|-----------------|
//! | stopwatch   | `stopwatch.rs`  |
//! | countdown   | `countdown.rs`  |
//! | alarm       | `alarm.rs`      |
//! | (drawing)   | `ui.rs`         |
//!
//! [`Column`] ties them together and converts to/from [`ColumnState`]
//! for saving.

mod alarm;
mod countdown;
mod stopwatch;
mod ui;

pub use ui::Ctx;

use alarm::Alarm;
use countdown::Countdown;
use stopwatch::Stopwatch;

use crate::clock::{MS_PER_SECOND, today_dmy};
use crate::persistence::{ColumnState, Sections};
use crate::sound;

/// Something the column asks the window to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnEvent {
    /// The stopwatch was started ("one at a time" then pauses the others).
    Started,
    /// The user confirmed deleting this column.
    RemoveRequested,
}

pub struct Column {
    /// Unique per run; tells columns apart (widget ids, removal).
    id: u64,
    stopwatch: Stopwatch,
    countdown: Countdown,
    alarm: Alarm,
    project: String,
    developer: String,
    sections: Sections,
    /// Widget-only state that isn't saved (typed text, edit modes, dialogs).
    ui: ui::UiState,
}

impl Drop for Column {
    /// A removed column must not leave its alarm ringing in the background.
    fn drop(&mut self) {
        self.alarm.silence();
    }
}

impl Column {
    /// A new, empty column showing only the stopwatch.
    pub fn new(id: u64) -> Self {
        Self::from_state(id, &ColumnState::default())
    }

    /// Rebuilds a column from saved state. The stopwatch always loads paused.
    pub fn from_state(id: u64, state: &ColumnState) -> Self {
        let elapsed_ms =
            i64::try_from(state.elapsed_secs).unwrap_or(i64::MAX / MS_PER_SECOND) * MS_PER_SECOND;
        Self {
            id,
            stopwatch: Stopwatch::paused_at(elapsed_ms),
            countdown: Countdown::default(),
            alarm: Alarm::new(),
            project: state.project.clone(),
            developer: state.developer.clone(),
            sections: state.sections,
            ui: ui::UiState::default(),
        }
    }

    /// What gets saved. Doesn't pause a running stopwatch (see
    /// [`Column::pause`] for that).
    pub fn to_state(&self, now: i64) -> ColumnState {
        ColumnState {
            elapsed_secs: u64::try_from(self.stopwatch.elapsed_ms(now) / MS_PER_SECOND)
                .unwrap_or(0),
            project: self.project.clone(),
            developer: self.developer.clone(),
            sections: self.sections,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    /// Pauses the stopwatch ("Stop all", "one at a time", quitting).
    pub fn pause(&mut self, now: i64) {
        self.stopwatch.stop(now);
    }

    /// Time-driven events. Called several times per second, also while the
    /// window is hidden in the tray, so alarms ring even then.
    pub fn tick(&mut self, now: i64, local_time: &str) {
        if self.countdown.check_finished(now) {
            sound::play_countdown_end();
        }
        self.alarm.check(local_time);
    }

    /// Text for the "Copy" button: one tab-separated row
    /// (developer, time, date, project) that pastes into a spreadsheet.
    fn clipboard_row(&self, now: i64) -> String {
        format!(
            "{}\t{}\t{}\t{} - ",
            self.developer,
            self.stopwatch.text(now),
            today_dmy(),
            self.project
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_roundtrip() {
        let mut column = Column::new(1);
        column.project = "website".into();
        column.sections.alarm = true;
        column.stopwatch.set_and_play(61_500, 0);

        let state = column.to_state(0);
        assert_eq!(state.elapsed_secs, 61, "milliseconds are dropped");

        let loaded = Column::from_state(2, &state);
        assert_eq!(loaded.project, "website");
        assert!(loaded.sections.alarm);
        assert!(!loaded.stopwatch.is_running(), "loads paused");
        assert_eq!(loaded.stopwatch.text(999_999), "00:01:01");
    }

    #[test]
    fn to_state_keeps_running() {
        let mut column = Column::new(1);
        column.stopwatch.play(0);
        assert_eq!(column.to_state(3_000).elapsed_secs, 3);
        assert!(column.stopwatch.is_running());
    }

    #[test]
    fn pause_stops_stopwatch() {
        let mut column = Column::new(1);
        column.stopwatch.play(0);
        column.pause(5_000);
        assert!(!column.stopwatch.is_running());
        assert_eq!(column.to_state(60_000).elapsed_secs, 5);
    }
}

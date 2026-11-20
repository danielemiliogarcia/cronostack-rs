//! Drawing a column.
//!
//! egui is immediate mode: [`Column::ui`] runs every frame, draws the
//! widgets and handles their clicks in the same pass. Each section has its
//! own function, in the order they appear on screen.

use egui::{FontFamily, FontId, Key, RichText, Vec2};

use super::countdown::CountdownState;
use super::stopwatch::StopwatchState;
use super::{Column, ColumnEvent};
use crate::clock::{hours_minutes_to_ms, local_time_hms};
use crate::theme::Palette;
use crate::widgets::{self, Icons};

/// Width of one column, in points.
const COLUMN_WIDTH: f32 = 210.0;

/// Widget state that isn't saved: text being typed, edit modes, dialogs.
#[derive(Default)]
pub(super) struct UiState {
    editing_project: bool,
    editing_developer: bool,
    alarm_editor: TimeEditor,
    stopwatch_editor: TimeEditor,
    countdown_editor: TimeEditor,
    confirm_remove: bool,
}

/// What a section needs to draw itself.
pub struct Ctx<'a> {
    pub icons: &'a Icons,
    pub palette: &'a Palette,
    pub now: i64,
}

impl Column {
    /// Draws the whole column and returns an event if the window must react.
    pub fn ui(&mut self, ui: &mut egui::Ui, cx: &Ctx<'_>) -> Option<ColumnEvent> {
        ui.set_width(COLUMN_WIDTH);
        let mut event = None;

        self.header(ui, cx);
        if self.sections.settings {
            self.settings_section(ui, cx);
        }
        if self.sections.project {
            self.project_section(ui, cx);
        }
        if self.sections.developer {
            self.developer_section(ui, cx);
        }
        if self.sections.clock {
            clock_section(ui, cx);
        }
        if self.sections.alarm {
            self.alarm_section(ui, cx);
        }
        if self.sections.stopwatch {
            event = event.or(self.stopwatch_section(ui, cx));
        }
        if self.sections.countdown {
            self.countdown_section(ui, cx);
        }
        if self.ui.confirm_remove {
            event = event.or(self.confirm_remove_dialog(ui));
        }

        event
    }

    /// Remove button and the "settings" toggle.
    fn header(&mut self, ui: &mut egui::Ui, cx: &Ctx<'_>) {
        ui.horizontal(|ui| {
            let remove = ui
                .add(widgets::icon_button(&cx.icons.close).frame(false))
                .on_hover_text("Remove this column")
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if remove.clicked() {
                self.ui.confirm_remove = true;
            }
            ui.checkbox(&mut self.sections.settings, small("settings"));
        });
    }

    /// Which sections to show, plus the "Copy" button.
    fn settings_section(&mut self, ui: &mut egui::Ui, cx: &Ctx<'_>) {
        widgets::group(ui, None, |ui| {
            egui::Grid::new(("settings", self.id))
                .num_columns(2)
                .spacing([12.0, 2.0])
                .show(ui, |ui| {
                    let s = &mut self.sections;
                    ui.checkbox(&mut s.project, small("project"));
                    ui.checkbox(&mut s.developer, small("developer"));
                    ui.end_row();
                    ui.checkbox(&mut s.clock, small("clock"));
                    ui.checkbox(&mut s.alarm, small("alarm"));
                    ui.end_row();
                    ui.checkbox(&mut s.stopwatch, small("stopwatch"));
                    ui.checkbox(&mut s.countdown, small("countdown"));
                    ui.end_row();
                });
            ui.separator();
            let copy = ui
                .button("Copy")
                .on_hover_text("Copy developer, time, date and project as a spreadsheet row");
            if copy.clicked() {
                ui.ctx().copy_text(self.clipboard_row(cx.now));
            }
        });
    }

    fn project_section(&mut self, ui: &mut egui::Ui, cx: &Ctx<'_>) {
        widgets::group(ui, Some("PROJECT"), |ui| {
            let id = ("project", self.id);
            click_to_edit(
                ui,
                cx.palette,
                &mut self.project,
                &mut self.ui.editing_project,
                id,
            );
        });
    }

    fn developer_section(&mut self, ui: &mut egui::Ui, cx: &Ctx<'_>) {
        widgets::group(ui, Some("DEVELOPER"), |ui| {
            let id = ("developer", self.id);
            click_to_edit(
                ui,
                cx.palette,
                &mut self.developer,
                &mut self.ui.editing_developer,
                id,
            );
        });
    }

    /// Alarm time + "on" checkbox, then the editor to change the time.
    fn alarm_section(&mut self, ui: &mut egui::Ui, cx: &Ctx<'_>) {
        ui.horizontal_top(|ui| {
            // Fixed width so the box doesn't push the column wider.
            let box_size = Vec2::new(COLUMN_WIDTH - 52.0, 0.0);
            ui.allocate_ui_with_layout(
                box_size,
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    widgets::group(ui, Some("ALARM"), |ui| {
                        ui.vertical_centered(|ui| {
                            let text = RichText::new(self.alarm.time_text()).strong().size(14.0);
                            // Dimmed while the alarm is off.
                            let text = if self.alarm.is_enabled() {
                                text.color(cx.palette.clock_text)
                            } else {
                                text.weak()
                            };
                            ui.label(text);
                        });
                    });
                },
            );

            let mut enabled = self.alarm.is_enabled();
            if ui.checkbox(&mut enabled, "on").changed() {
                self.alarm.set_enabled(enabled);
            }
        });

        if let Some(time_ms) = self.ui.alarm_editor.show(ui, ("alarm", self.id)) {
            self.alarm.set_time(time_ms);
        }
        ui.separator();
    }

    /// Time editor, display, and play / pause / reset buttons.
    fn stopwatch_section(&mut self, ui: &mut egui::Ui, cx: &Ctx<'_>) -> Option<ColumnEvent> {
        let now = cx.now;
        let mut started = false;

        if let Some(time_ms) = self.ui.stopwatch_editor.show(ui, ("stopwatch", self.id)) {
            started |= self.stopwatch.set_and_play(time_ms, now);
        }

        let color = match self.stopwatch.state(now) {
            StopwatchState::Running => cx.palette.stopwatch_running,
            StopwatchState::Paused => cx.palette.stopwatch_paused,
            StopwatchState::Zero => cx.palette.stopwatch_zero,
        };
        widgets::digital_display(ui, cx.palette, &self.stopwatch.text(now), color);

        ui.horizontal(|ui| {
            let running = self.stopwatch.is_running();
            if ui
                .add_enabled(!running, widgets::icon_button(&cx.icons.play))
                .on_hover_text("Start")
                .clicked()
            {
                started |= self.stopwatch.play(now);
            }
            if ui
                .add_enabled(running, widgets::icon_button(&cx.icons.stop))
                .on_hover_text("Pause")
                .clicked()
            {
                self.stopwatch.stop(now);
            }
            if ui
                .add(widgets::icon_button(&cx.icons.reset))
                .on_hover_text("Reset")
                .clicked()
            {
                self.stopwatch.reset();
            }
        });
        ui.separator();

        started.then_some(ColumnEvent::Started)
    }

    /// Time editor, display, and start / stop buttons.
    fn countdown_section(&mut self, ui: &mut egui::Ui, cx: &Ctx<'_>) {
        let now = cx.now;

        if let Some(time_ms) = self.ui.countdown_editor.show(ui, ("countdown", self.id)) {
            self.countdown.set(time_ms, now);
        }

        let color = match self.countdown.state() {
            CountdownState::Idle => cx.palette.countdown_idle,
            CountdownState::Running => cx.palette.countdown_running,
            CountdownState::Paused => cx.palette.countdown_paused,
        };
        widgets::digital_display(ui, cx.palette, &self.countdown.text(now), color);

        ui.horizontal(|ui| {
            let can_start = self.countdown.can_play();
            if ui
                .add_enabled(can_start, widgets::icon_button(&cx.icons.countdown))
                .on_hover_text("Start")
                .clicked()
            {
                self.countdown.play(now);
            }
            let running = self.countdown.is_running();
            if ui
                .add_enabled(running, widgets::icon_button(&cx.icons.stop))
                .on_hover_text("Stop")
                .clicked()
            {
                self.countdown.stop(now);
            }
        });
        ui.separator();
    }

    /// "Are you sure?" popup shown after clicking the remove button.
    fn confirm_remove_dialog(&mut self, ui: &mut egui::Ui) -> Option<ColumnEvent> {
        let mut event = None;
        let modal =
            egui::Modal::new(egui::Id::unique(("confirm_remove", self.id))).show(ui.ctx(), |ui| {
                ui.set_width(260.0);
                ui.heading("Remove this column?");
                ui.label("Its stopwatch time, project and developer will be lost.");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Remove").clicked() {
                        event = Some(ColumnEvent::RemoveRequested);
                        self.ui.confirm_remove = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.ui.confirm_remove = false;
                    }
                });
            });
        // Escape or a click outside the popup cancels.
        if modal.should_close() {
            self.ui.confirm_remove = false;
        }
        event
    }
}

/// "LOCAL TIME" box with the current time of day.
fn clock_section(ui: &mut egui::Ui, cx: &Ctx<'_>) {
    widgets::group(ui, Some("LOCAL TIME"), |ui| {
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(local_time_hms())
                    .strong()
                    .size(14.0)
                    .color(cx.palette.clock_text),
            );
        });
    });
}

fn small(text: &str) -> RichText {
    RichText::new(text).size(10.0)
}

/// The "[x] hh : mm [set]" row used by the alarm, stopwatch and countdown.
///
/// The fields stay disabled until the checkbox is ticked, so a time isn't
/// changed by accident. "set" (or Enter in the minutes field) applies it.
#[derive(Default)]
struct TimeEditor {
    unlocked: bool,
    hours: String,
    minutes: String,
}

impl TimeEditor {
    /// Draws the row. Returns the entered time in milliseconds when the user
    /// applies it; the row is then cleared and locked again.
    fn show(&mut self, ui: &mut egui::Ui, id: (&'static str, u64)) -> Option<i64> {
        let mut apply = false;

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.unlocked, "");
            let unlocked = self.unlocked;

            let hours = egui::TextEdit::singleline(&mut self.hours)
                .desired_width(38.0)
                .hint_text("hh");
            ui.add_enabled(unlocked, hours.id_salt((id, "hours")));
            ui.label(":");
            let minutes = egui::TextEdit::singleline(&mut self.minutes)
                .desired_width(38.0)
                .hint_text("mm");
            let minutes = ui.add_enabled(unlocked, minutes.id_salt((id, "minutes")));

            let enter_pressed = minutes.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            let set_clicked = ui.add_enabled(unlocked, egui::Button::new("set")).clicked();
            apply = unlocked && (enter_pressed || set_clicked);
        });

        if !apply {
            return None;
        }
        let time_ms = hours_minutes_to_ms(&self.hours, &self.minutes);
        *self = TimeEditor::default();
        Some(time_ms)
    }
}

/// Shows the text on a tinted button; clicking turns it into a text box.
/// Enter or clicking elsewhere ends editing.
fn click_to_edit(
    ui: &mut egui::Ui,
    palette: &Palette,
    text: &mut String,
    editing: &mut bool,
    id: (&'static str, u64),
) {
    let font = FontId::new(13.0, FontFamily::Proportional);

    if *editing {
        let response = ui.add(
            egui::TextEdit::singleline(text)
                .id_salt(id)
                .font(font)
                .align(egui::Align2::CENTER_CENTER)
                .desired_width(f32::INFINITY),
        );
        // Put the cursor in the box as soon as it appears.
        if !response.has_focus() && !response.lost_focus() {
            response.request_focus();
        }
        if response.lost_focus() {
            *editing = false;
        }
    } else {
        // A space keeps the button its normal height when the text is empty.
        let shown = if text.is_empty() { " " } else { text.as_str() };
        let button = egui::Button::new(
            RichText::new(shown)
                .font(font)
                .strong()
                .color(palette.field_text),
        )
        .fill(palette.field_fill)
        .min_size(Vec2::new(ui.available_width(), 22.0));
        if ui.add(button).on_hover_text("Click to edit").clicked() {
            *editing = true;
        }
    }
}

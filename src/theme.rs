//! Look and feel: the "Classic" CronoStack colors, or the system theme.
//!
//! Widgets never hard-code colors; they ask for a [`Palette`], which is
//! built from the selected [`ThemeMode`] and the current egui visuals.

use egui::{Color32, Visuals};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    /// Light window, black LCD displays with bright digits, blue fields.
    #[default]
    Classic,
    /// Follows the OS light/dark setting, with matching display colors.
    System,
}

impl ThemeMode {
    pub const ALL: [ThemeMode; 2] = [ThemeMode::Classic, ThemeMode::System];

    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::Classic => "Classic",
            ThemeMode::System => "System",
        }
    }

    /// Tells egui which light/dark theme to use for the window.
    pub fn apply(self, ctx: &egui::Context) {
        let preference = match self {
            ThemeMode::Classic => egui::ThemePreference::Light,
            ThemeMode::System => egui::ThemePreference::System,
        };
        ctx.set_theme(preference);
    }
}

/// Colors used by the app's own widgets.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Background and border of the stopwatch/countdown displays.
    pub display_bg: Color32,
    pub display_border: Color32,

    /// Digits of the stopwatch.
    pub stopwatch_running: Color32,
    pub stopwatch_paused: Color32,
    pub stopwatch_zero: Color32,

    /// Digits of the countdown.
    pub countdown_running: Color32,
    pub countdown_paused: Color32,
    pub countdown_idle: Color32,

    /// Project/developer fields while not being edited.
    pub field_fill: Color32,
    pub field_text: Color32,

    /// Clock and alarm time.
    pub clock_text: Color32,
}

impl Palette {
    pub fn new(mode: ThemeMode, visuals: &Visuals) -> Self {
        match mode {
            ThemeMode::Classic => Self::CLASSIC,
            ThemeMode::System => Self::system(visuals),
        }
    }

    const CLASSIC: Palette = Palette {
        display_bg: Color32::from_gray(1),
        display_border: Color32::GRAY,
        stopwatch_running: Color32::from_rgb(0, 255, 151),
        stopwatch_paused: Color32::from_rgb(254, 131, 52),
        stopwatch_zero: Color32::WHITE,
        countdown_running: Color32::RED,
        countdown_paused: Color32::YELLOW,
        countdown_idle: Color32::GRAY,
        field_fill: Color32::from_rgb(172, 211, 224),
        field_text: Color32::BLACK,
        clock_text: Color32::from_gray(1),
    };

    /// Derived from the active (light or dark) egui visuals.
    fn system(v: &Visuals) -> Self {
        let green = if v.dark_mode {
            Color32::from_rgb(90, 210, 140)
        } else {
            Color32::from_rgb(20, 140, 70)
        };
        Palette {
            display_bg: v.extreme_bg_color,
            display_border: v.widgets.noninteractive.bg_stroke.color,
            stopwatch_running: green,
            stopwatch_paused: v.warn_fg_color,
            stopwatch_zero: v.strong_text_color(),
            countdown_running: v.error_fg_color,
            countdown_paused: v.warn_fg_color,
            countdown_idle: v.weak_text_color(),
            field_fill: v.widgets.inactive.weak_bg_fill,
            field_text: v.strong_text_color(),
            clock_text: v.strong_text_color(),
        }
    }
}

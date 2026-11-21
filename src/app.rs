//! The main window.
//!
//! ```text
//! [+] [Menu] [x] One at a time [v]   <- top bar (add, menu, option, hide;
//!                                        no hide button on the web)
//! ┌──────────┐ │ ┌──────────┐
//! │ column 1 │ │ │ column 2 │ ...    <- one column per stopwatch
//! └──────────┘ │ └──────────┘
//! ```
//!
//! eframe calls two methods of [`App`] every frame:
//! - [`eframe::App::logic`]: timers, tray commands, saving. Also runs while
//!   the window is hidden in the tray.
//! - [`eframe::App::ui`]: drawing (only while visible).
//!
//! The same app runs on the web (WebAssembly). There the window is the
//! browser page, so the tray, hiding and window resizing are left out.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

// `std::time::Instant` panics on the web; `web_time` works everywhere.
use web_time::Instant;

use egui::{Rect, RichText, Vec2, ViewportCommand};

use crate::clock::{local_time_hms, now_ms};
use crate::column::{self, Column, ColumnEvent};
use crate::persistence::{AppState, Store};
use crate::theme::{Palette, ThemeMode};
use crate::tray::{self, Tray, TrayCommand};
use crate::widgets::{self, Icons};

/// Space between the window border and the content, in points.
const MARGIN: i8 = 6;
/// Window width when there are no columns.
#[cfg(not(target_arch = "wasm32"))]
const MIN_WIDTH: f32 = 220.0;
/// Protects the times against crashes or the app being killed. On the web a
/// tab can be closed at any moment without a chance to save, so save often.
#[cfg(not(target_arch = "wasm32"))]
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(30);
#[cfg(target_arch = "wasm32")]
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(5);
/// Frames `--smoke-test` renders before checking saving and quitting.
const SMOKE_TEST_FRAMES: u32 = 30;

/// Set when `--smoke-test` succeeded; read by `main` after the window closes.
pub static SMOKE_TEST_PASSED: AtomicBool = AtomicBool::new(false);

pub struct App {
    store: Store,
    columns: Vec<Column>,
    next_column_id: u64,
    /// Starting a stopwatch pauses all the others.
    one_at_a_time: bool,
    theme: ThemeMode,

    icons: Icons,
    /// `None` when the desktop has no system tray.
    tray: Option<Tray>,
    /// Clicks on the tray icon/menu, sent from the tray's thread.
    tray_commands: Receiver<TrayCommand>,
    window_visible: bool,
    show_data_location: bool,
    show_about: bool,

    /// Last size we asked the window to be, to only resize on changes.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    last_requested_size: Option<Vec2>,
    saved_on_close: bool,
    last_autosave: Instant,
    /// `Some(frames rendered)` when running with `--smoke-test`.
    smoke_test_frames: Option<u32>,
}

impl App {
    /// Called once at startup by eframe.
    pub fn new(cc: &eframe::CreationContext<'_>, store: Store, smoke_test: bool) -> Self {
        let ctx = &cc.egui_ctx;
        widgets::install_fonts(ctx);

        let (tray_sender, tray_commands) = mpsc::channel();
        let tray = tray::create(ctx, tray_sender);
        if tray.is_none() && cfg!(not(target_arch = "wasm32")) {
            log::info!("no system tray available; the hide button will minimize instead");
        }

        #[cfg(not(target_arch = "wasm32"))]
        spawn_ticker(ctx.clone());

        log::info!("state: {}", store.location());
        let state = store.load_or_default();
        state.theme.apply(ctx);
        let columns: Vec<Column> = state
            .columns
            .iter()
            .zip(0..)
            .map(|(column, id)| Column::from_state(id, column))
            .collect();

        Self {
            store,
            next_column_id: columns.len() as u64,
            columns,
            one_at_a_time: state.one_at_a_time,
            theme: state.theme,
            icons: Icons::load(ctx),
            tray,
            tray_commands,
            window_visible: true,
            show_data_location: false,
            show_about: false,
            last_requested_size: None,
            saved_on_close: false,
            last_autosave: Instant::now(),
            smoke_test_frames: smoke_test.then_some(0),
        }
    }

    // ------------------------------------------------------------- actions

    fn add_column(&mut self) {
        self.columns.push(Column::new(self.next_column_id));
        self.next_column_id += 1;
    }

    fn pause_all(&mut self) {
        let now = now_ms();
        self.columns.iter_mut().for_each(|c| c.pause(now));
    }

    fn pause_all_except(&mut self, id: u64) {
        let now = now_ms();
        self.columns
            .iter_mut()
            .filter(|c| c.id() != id)
            .for_each(|c| c.pause(now));
    }

    fn state(&self) -> AppState {
        let now = now_ms();
        AppState {
            theme: self.theme,
            one_at_a_time: self.one_at_a_time,
            columns: self.columns.iter().map(|c| c.to_state(now)).collect(),
        }
    }

    fn save(&mut self) {
        if let Err(e) = self.store.save(&self.state()) {
            log::error!("{e}");
        }
        self.last_autosave = Instant::now();
    }

    /// On quit, stopwatches are paused so no time is counted while closed.
    fn pause_all_and_save(&mut self) {
        self.pause_all();
        self.save();
        self.saved_on_close = true;
    }

    fn set_theme(&mut self, ctx: &egui::Context, theme: ThemeMode) {
        self.theme = theme;
        theme.apply(ctx);
    }

    /// Hide to / show from the tray. Without a tray we minimize instead,
    /// otherwise there'd be no way to bring the window back.
    fn set_window_visible(&mut self, ctx: &egui::Context, visible: bool) {
        self.window_visible = visible;
        if self.tray.is_some() {
            ctx.send_viewport_cmd(ViewportCommand::Visible(visible));
            if visible {
                ctx.send_viewport_cmd(ViewportCommand::Focus);
            }
        } else {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(!visible));
        }
    }

    fn handle_tray_command(&mut self, ctx: &egui::Context, command: TrayCommand) {
        match command {
            TrayCommand::Toggle => self.set_window_visible(ctx, !self.window_visible),
            TrayCommand::PauseAll => self.pause_all(),
            TrayCommand::Exit => {
                self.pause_all_and_save();
                std::process::exit(0);
            }
        }
    }

    /// `--smoke-test`: after a few frames, check that a column can be added
    /// and that the state survives a save + load, then quit.
    fn finish_smoke_test(&mut self, ctx: &egui::Context) {
        self.add_column();
        let expected = self.state();
        let result = self
            .store
            .save(&expected)
            .map_err(|e| e.to_string())
            .and_then(|()| {
                let loaded = self.store.load().map_err(|e| e.to_string())?;
                if loaded == expected {
                    Ok(())
                } else {
                    Err("saved state did not load back unchanged".to_owned())
                }
            });
        match result {
            Ok(()) => {
                println!(
                    "smoke test passed: window rendered {SMOKE_TEST_FRAMES} frames, state saved and loaded"
                );
                SMOKE_TEST_PASSED.store(true, Ordering::SeqCst);
            }
            Err(e) => eprintln!("smoke test failed: {e}"),
        }
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }

    // ------------------------------------------------------------- drawing

    /// The row at the top of the window. Returns the area it used.
    fn top_bar(&mut self, ui: &mut egui::Ui) -> Rect {
        ui.horizontal(|ui| {
            if ui
                .add(widgets::icon_button(&self.icons.add))
                .on_hover_text("Add a column")
                .clicked()
            {
                self.add_column();
            }
            ui.menu_button("Menu", |ui| self.main_menu(ui));
            ui.checkbox(&mut self.one_at_a_time, "One at a time")
                .on_hover_text("Starting a stopwatch pauses all the others");
            // On the web there's no window to hide.
            if cfg!(target_arch = "wasm32") {
                return;
            }
            ui.add_space(8.0);
            let tip = if self.tray.is_some() {
                "Hide to the system tray"
            } else {
                "Minimize"
            };
            if ui
                .add(widgets::icon_button(&self.icons.hide))
                .on_hover_text(tip)
                .clicked()
            {
                let ctx = ui.ctx().clone();
                self.set_window_visible(&ctx, false);
            }
        })
        .response
        .rect
    }

    fn main_menu(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Theme").weak());
        for mode in ThemeMode::ALL {
            if ui.radio(self.theme == mode, mode.label()).clicked() {
                let ctx = ui.ctx().clone();
                self.set_theme(&ctx, mode);
            }
        }
        ui.separator();
        if ui.button("Pause all").clicked() {
            self.pause_all();
        }
        if ui.button("Where is my data?").clicked() {
            self.show_data_location = true;
        }
        if ui.button("About CronoStack").clicked() {
            self.show_about = true;
        }
    }

    /// All columns side by side with thin dividers. Returns the area used.
    fn columns(&mut self, ui: &mut egui::Ui, now: i64) -> Rect {
        if self.columns.is_empty() {
            return ui
                .label(RichText::new("Press + to add a column").weak())
                .rect;
        }

        let palette = Palette::new(self.theme, ui.visuals());
        let cx = column::Ctx {
            icons: &self.icons,
            palette: &palette,
            now,
        };
        let mut used = Rect::NOTHING;
        let mut events = Vec::new();
        let mut divider_xs = Vec::new();

        ui.horizontal_top(|ui| {
            for (i, column) in self.columns.iter_mut().enumerate() {
                if i > 0 {
                    ui.add_space(4.0);
                }
                let response = ui.vertical(|ui| column.ui(ui, &cx));
                if let Some(event) = response.inner {
                    events.push((column.id(), event));
                }
                let rect = response.response.rect;
                if i > 0 {
                    divider_xs.push((used.right() + rect.left()) / 2.0);
                }
                used = used.union(rect);
            }
        });

        // Dividers are painted after measuring: a `ui.separator()` would
        // stretch to the full window height and stop the window shrinking.
        let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
        for x in divider_xs {
            ui.painter().vline(x, used.y_range(), stroke);
        }

        for (id, event) in events {
            match event {
                ColumnEvent::Started if self.one_at_a_time => self.pause_all_except(id),
                ColumnEvent::Started => {}
                ColumnEvent::RemoveRequested => self.columns.retain(|c| c.id() != id),
            }
        }

        used
    }

    fn data_location_dialog(&mut self, ctx: &egui::Context) {
        let location = self.store.location();
        let modal = egui::Modal::new(egui::Id::unique("data_location")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.heading("Where is my data?");
            ui.label("Your columns and settings are saved in:");
            ui.add(egui::Label::new(RichText::new(&location).monospace()).wrap());
            if cfg!(target_arch = "wasm32") {
                ui.label(
                    RichText::new(
                        "Saved every 5 seconds. It stays in this browser on this device; \
                         clearing the site's data erases it.",
                    )
                    .weak(),
                );
            } else {
                ui.label(RichText::new("Saved on exit and every 30 seconds.").weak());
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    if ui.button("Copy path").clicked() {
                        ui.ctx().copy_text(location.clone());
                    }
                    if let Some(dir) = self.store.folder()
                        && ui.button("Open folder").clicked()
                    {
                        open_in_file_manager(dir);
                    }
                }
                if ui.button("Close").clicked() {
                    self.show_data_location = false;
                }
            });
        });
        if modal.should_close() {
            self.show_data_location = false;
        }
    }

    fn about_dialog(&mut self, ctx: &egui::Context) {
        let modal = egui::Modal::new(egui::Id::unique("about")).show(ctx, |ui| {
            ui.heading("CronoStack");
            ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
            ui.label("Stopwatches, countdowns and alarms, side by side.");
            ui.label(RichText::new("by EE & FF").weak());
            ui.add_space(4.0);
            ui.hyperlink_to("Source code", env!("CARGO_PKG_REPOSITORY"));
            ui.label(
                RichText::new(format!("License: {}", env!("CARGO_PKG_LICENSE")))
                    .small()
                    .weak(),
            );
            ui.add_space(8.0);
            if ui.button("Close").clicked() {
                self.show_about = false;
            }
        });
        if modal.should_close() {
            self.show_about = false;
        }
    }

    /// Resizes the window to fit `content` (in window coordinates).
    #[cfg(not(target_arch = "wasm32"))]
    fn fit_window_to(&mut self, ctx: &egui::Context, content: Rect) {
        // Content starts at the top-left margin; add it once more for the
        // right and bottom.
        let size = content.max.to_vec2() + Vec2::splat(f32::from(MARGIN));
        let size = size.max(Vec2::new(MIN_WIDTH, 0.0)).ceil();
        if self.last_requested_size != Some(size) {
            self.last_requested_size = Some(size);
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = now_ms();
        let local_time = local_time_hms();
        for column in &mut self.columns {
            column.tick(now, &local_time);
        }

        while let Ok(command) = self.tray_commands.try_recv() {
            self.handle_tray_command(ctx, command);
        }

        let closing = ctx.input(|i| i.viewport().close_requested());
        if closing && !self.saved_on_close {
            self.pause_all_and_save();
        } else if self.last_autosave.elapsed() >= AUTOSAVE_INTERVAL {
            self.save();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let now = now_ms();
        let panel_frame = egui::Frame::central_panel(ui.style()).inner_margin(MARGIN);

        // The panel's own size is always the whole window, so the content
        // size is measured from the widgets instead.
        let content = egui::CentralPanel::no_frame()
            .frame(panel_frame)
            .show(ui, |ui| {
                let top_bar = self.top_bar(ui);
                ui.add_space(2.0);
                top_bar.union(self.columns(ui, now))
            })
            .inner;

        let ctx = ui.ctx().clone();
        if self.show_data_location {
            self.data_location_dialog(&ctx);
        }
        if self.show_about {
            self.about_dialog(&ctx);
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.fit_window_to(&ctx, content);
        #[cfg(target_arch = "wasm32")]
        let _ = content; // the page decides the size on the web

        if let Some(frames) = &mut self.smoke_test_frames {
            *frames += 1;
            if *frames == SMOKE_TEST_FRAMES {
                self.finish_smoke_test(&ctx);
            }
            ctx.request_repaint();
        }

        // Redraw right after the next full second so the clocks tick.
        let ms_to_next_second = 1000 - now.rem_euclid(1000).unsigned_abs() + 5;
        ctx.request_repaint_after(Duration::from_millis(ms_to_next_second));
    }
}

/// Wakes the app 4 times per second from a background thread, so
/// [`eframe::App::logic`] keeps running (and alarms ring) while the window
/// is hidden and nothing else would trigger a frame.
#[cfg(not(target_arch = "wasm32"))]
fn spawn_ticker(ctx: egui::Context) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(250));
            ctx.request_repaint();
        }
    });
}

/// Opens a folder in the platform's file manager.
#[cfg(not(target_arch = "wasm32"))]
fn open_in_file_manager(dir: &std::path::Path) {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    if let Err(e) = std::process::Command::new(program).arg(dir).spawn() {
        log::error!("could not open {}: {e}", dir.display());
    }
}

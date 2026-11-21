//! CronoStack: stopwatches side by side, each with an optional countdown,
//! alarm, clock and project/developer labels.
//!
//! | module        | what it does |
//! |---------------|--------------|
//! | `desktop`     | desktop entry point: command line, native window |
//! | `web`         | web entry point: runs the app in a browser canvas |
//! | `app`         | main window: top bar, menu, columns, tray, saving |
//! | `column`      | one column (stopwatch, countdown, alarm and its UI) |
//! | `clock`       | time helpers (current time, `HH:MM:SS`) |
//! | `persistence` | save/load the state as TOML (file, or browser storage) |
//! | `theme`       | Classic or system colors |
//! | `sound`       | alarm and countdown sounds |
//! | `tray`        | system tray icon and menu (desktop) |
//! | `widgets`     | shared widgets, embedded icons and font |
//!
//! Logging: on the desktop set `RUST_LOG=cronostack=info`; on the web, logs go
//! to the browser console.

// On Windows release builds, don't open a console window next to the app.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod clock;
mod column;
mod persistence;
mod sound;
mod theme;
mod tray;
mod widgets;

#[cfg(not(target_arch = "wasm32"))]
mod desktop;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    desktop::main()
}

#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

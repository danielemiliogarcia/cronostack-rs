//! System tray icon with "Show / Hide", "Stop All" and "Exit".
//!
//! On Linux the pure-Rust StatusNotifierItem (KSNI) backend is used, so no
//! GTK libraries are needed.

use std::sync::mpsc::Sender;

#[derive(Debug, Clone, Copy)]
pub enum TrayCommand {
    Toggle,
    PauseAll,
    Exit,
}

#[cfg(feature = "tray")]
pub struct Tray {
    _icon: tray_icon::TrayIcon,
}

#[cfg(feature = "tray")]
pub fn create(ctx: &egui::Context, tx: Sender<TrayCommand>) -> Option<Tray> {
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let img = if cfg!(windows) {
        crate::widgets::decode_png(include_bytes!("../assets/icons/tray_windows.png"))
    } else {
        crate::widgets::decode_png(include_bytes!("../assets/icons/tray.png"))
    };
    let icon = Icon::from_rgba(
        img.as_raw().to_vec(),
        img.size[0] as u32,
        img.size[1] as u32,
    )
    .ok()?;

    let show = MenuItem::new("Show / Hide", true, None);
    let pause = MenuItem::new("Pause all", true, None);
    let exit = MenuItem::new("Exit", true, None);
    let menu = Menu::new();
    menu.append_items(&[&show, &pause, &PredefinedMenuItem::separator(), &exit])
        .ok()?;

    let tray = TrayIconBuilder::new()
        .with_icon(icon)
        .with_tooltip("Click to show or hide CronoStack")
        .with_title("CronoStack")
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .map_err(|e| log::warn!("could not create the tray icon: {e}"))
        .ok()?;

    let (show_id, pause_id, exit_id) = (show.id().clone(), pause.id().clone(), exit.id().clone());
    {
        let (ctx, tx) = (ctx.clone(), tx.clone());
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            let cmd = if e.id == show_id {
                TrayCommand::Toggle
            } else if e.id == pause_id {
                TrayCommand::PauseAll
            } else if e.id == exit_id {
                TrayCommand::Exit
            } else {
                return;
            };
            let _ = tx.send(cmd);
            ctx.request_repaint();
        }));
    }
    {
        let ctx = ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            let toggle = match e {
                // Windows reports double clicks, the usual way to restore a tray app there
                TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } => true,
                // Linux (KSNI) / macOS only report single clicks
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => !cfg!(windows),
                _ => false,
            };
            if toggle {
                let _ = tx.send(TrayCommand::Toggle);
                ctx.request_repaint();
            }
        }));
    }

    Some(Tray { _icon: tray })
}

#[cfg(not(feature = "tray"))]
pub struct Tray;

#[cfg(not(feature = "tray"))]
pub fn create(_ctx: &egui::Context, _tx: Sender<TrayCommand>) -> Option<Tray> {
    None
}

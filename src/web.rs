//! Web entry point: runs the app inside the page's `<canvas>` (see
//! `index.html`). State is kept in the browser's local storage.

use wasm_bindgen::JsCast;

use crate::{app, persistence};

/// The `id` of the canvas in `index.html`.
const CANVAS_ID: &str = "cronostack_canvas";

pub fn start() {
    // Logs go to the browser's developer console.
    let _ = eframe::WebLogger::init(log::LevelFilter::Info);

    wasm_bindgen_futures::spawn_local(async {
        let Some(canvas) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(CANVAS_ID))
            .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
        else {
            log::error!("no <canvas id=\"{CANVAS_ID}\"> in the page");
            return;
        };

        let result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| {
                    Ok(Box::new(app::App::new(
                        cc,
                        persistence::Store::locate(),
                        false,
                    )))
                }),
            )
            .await;
        if let Err(e) = result {
            log::error!("failed to start CronoStack: {e:?}");
        }
    });
}

//! Desktop entry point: command-line options, `--smoke-test`, and the
//! native window.

use crate::{app, persistence, widgets};

use std::process::ExitCode;
use std::sync::atomic::Ordering;
use std::time::Duration;

const HELP: &str = "\
CronoStack: stopwatches, countdowns and alarms, side by side.

Usage: cronostack [OPTION]

Options:
  --smoke-test  Open the window, render a few frames, check saving, and exit
                (0 = success). Uses a temporary data folder.
  -V, --version Print the version
  -h, --help    Print this help

Environment:
  CRONOSTACK_DATA_DIR  Folder for the state file (default: the OS data folder)
  RUST_LOG             Log filter, e.g. cronostack=info
";

/// Longest a `--smoke-test` may take before it's considered hung.
const SMOKE_TEST_TIMEOUT: Duration = Duration::from_secs(120);

pub fn main() -> ExitCode {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("error,cronostack=warn"),
    )
    .init();

    let mut smoke_test = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-V" | "--version" => {
                print_ignoring_errors(&format!("cronostack {}\n", env!("CARGO_PKG_VERSION")));
                return ExitCode::SUCCESS;
            }
            "-h" | "--help" => {
                print_ignoring_errors(HELP);
                return ExitCode::SUCCESS;
            }
            "--smoke-test" => smoke_test = true,
            other => {
                eprintln!("unknown option: {other}\n\n{HELP}");
                return ExitCode::from(2);
            }
        }
    }

    if smoke_test {
        run_smoke_test()
    } else {
        match run(persistence::Store::locate(), false) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("cronostack: {e}");
                ExitCode::FAILURE
            }
        }
    }
}

/// Writes to stdout without panicking if it's closed. On Windows the app is
/// a GUI-subsystem program, so a caller may not be listening (`println!`
/// would panic with "failed printing to stdout").
fn print_ignoring_errors(text: &str) {
    use std::io::Write;
    let mut stdout = std::io::stdout().lock();
    let _ = stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush());
}

/// Runs the real window against a throwaway data folder (used by CI to
/// prove each release binary starts and renders on its platform).
fn run_smoke_test() -> ExitCode {
    let dir = std::env::temp_dir().join(format!("cronostack-smoke-{}", std::process::id()));
    std::thread::spawn(|| {
        std::thread::sleep(SMOKE_TEST_TIMEOUT);
        eprintln!("smoke test failed: no result after {SMOKE_TEST_TIMEOUT:?}");
        std::process::exit(1);
    });

    let result = run(persistence::Store::in_dir(&dir), true);
    let _ = std::fs::remove_dir_all(&dir);
    match result {
        Ok(()) if app::SMOKE_TEST_PASSED.load(Ordering::SeqCst) => ExitCode::SUCCESS,
        Ok(()) => {
            eprintln!("smoke test failed: the window closed before the test finished");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("smoke test failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(store: persistence::Store, smoke_test: bool) -> eframe::Result {
    let icon = widgets::decode_png(include_bytes!("../assets/icons/app.png"));
    let icon = egui::IconData {
        rgba: icon.as_raw().to_vec(),
        width: icon.size[0] as u32,
        height: icon.size[1] as u32,
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("CronoStack")
            .with_app_id("cronostack")
            .with_icon(icon)
            // Initial size only: the app resizes the window to fit its content.
            .with_inner_size([230.0, 260.0]),
        ..Default::default()
    };

    eframe::run_native(
        "CronoStack",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, store, smoke_test)))),
    )
}

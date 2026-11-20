//! Sound playback for the alarm and the end of a countdown.
//!
//! With the `native-audio` feature sounds are played through `rodio`.
//! Without it (the default, so no ALSA dev headers are needed to build) the
//! embedded WAV is written to a temp file and played with a system player.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub enum Sound {
    Alarm,
    CountdownEnd,
}

impl Sound {
    fn bytes(self) -> &'static [u8] {
        match self {
            Sound::Alarm => include_bytes!("../assets/sounds/alarm.wav"),
            Sound::CountdownEnd => include_bytes!("../assets/sounds/countdown_end.wav"),
        }
    }
    #[cfg_attr(feature = "native-audio", allow(dead_code))]
    fn file_name(self) -> &'static str {
        match self {
            Sound::Alarm => "cronostack-alarm.wav",
            Sound::CountdownEnd => "cronostack-countdown-end.wav",
        }
    }
}

/// Plays `sound` once, blocking until it ends or `stop()` returns true.
pub fn play_blocking(sound: Sound, stop: &dyn Fn() -> bool) -> Result<(), String> {
    backend::play(sound, stop)
}

#[cfg(feature = "native-audio")]
mod backend {
    use super::*;
    use std::io::Cursor;

    pub fn play(sound: Sound, stop: &dyn Fn() -> bool) -> Result<(), String> {
        let sink = rodio::DeviceSinkBuilder::open_default_sink().map_err(|e| e.to_string())?;
        let player = rodio::Player::connect_new(sink.mixer());
        let source =
            rodio::Decoder::try_from(Cursor::new(sound.bytes())).map_err(|e| e.to_string())?;
        player.append(source);
        while !player.empty() {
            if stop() {
                player.stop();
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        Ok(())
    }
}

#[cfg(not(feature = "native-audio"))]
mod backend {
    use super::*;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    fn temp_file(sound: Sound) -> Result<PathBuf, String> {
        let path = std::env::temp_dir().join(sound.file_name());
        let up_to_date = std::fs::metadata(&path)
            .map(|m| m.len() == sound.bytes().len() as u64)
            .unwrap_or(false);
        if !up_to_date {
            std::fs::write(&path, sound.bytes()).map_err(|e| e.to_string())?;
        }
        Ok(path)
    }

    fn candidates(path: &str) -> Vec<Command> {
        let mk = |prog: &str, args: &[&str]| {
            let mut c = Command::new(prog);
            c.args(args).arg(path);
            c
        };
        if cfg!(target_os = "macos") {
            vec![mk("afplay", &[])]
        } else if cfg!(windows) {
            let mut c = Command::new("powershell");
            c.args(["-NoProfile", "-NonInteractive", "-Command"])
                .arg(format!(
                    "(New-Object Media.SoundPlayer '{}').PlaySync()",
                    path.replace('\'', "''")
                ));
            vec![c]
        } else {
            vec![
                mk("paplay", &[]),
                mk("pw-play", &[]),
                mk("aplay", &["-q"]),
                mk("ffplay", &["-nodisp", "-autoexit", "-loglevel", "quiet"]),
            ]
        }
    }

    pub fn play(sound: Sound, stop: &dyn Fn() -> bool) -> Result<(), String> {
        let path = temp_file(sound)?;
        let path = path.to_string_lossy();
        for mut cmd in candidates(&path) {
            let Ok(mut child) = cmd
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            else {
                continue; // player not installed, try next one
            };
            loop {
                if stop() {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ok(());
                }
                match child.try_wait() {
                    Ok(Some(status)) if status.success() => return Ok(()),
                    Ok(Some(status)) => return Err(format!("player exited with {status}")),
                    Ok(None) => thread::sleep(Duration::from_millis(50)),
                    Err(e) => return Err(e.to_string()),
                }
            }
        }
        Err(
            "no audio player found (tried paplay, pw-play, aplay, ffplay / afplay / powershell)"
                .into(),
        )
    }
}

/// Countdown end sound: played once in background, retrying if the sound
/// device is briefly busy (up to 50 attempts).
pub fn play_countdown_end() {
    thread::spawn(|| {
        for attempt in 1..=50 {
            match play_blocking(Sound::CountdownEnd, &|| false) {
                Ok(()) => return,
                Err(e) => {
                    log::warn!("countdown sound failed (attempt {attempt}): {e}");
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
    });
}

/// Repeating alarm: beeps for one minute, waits four minutes, and repeats
/// until stopped.
#[derive(Clone, Default)]
pub struct AlarmPlayer {
    generation: Arc<AtomicU64>,
}

impl AlarmPlayer {
    const SOUND_FOR: Duration = Duration::from_secs(60);
    const PAUSE_FOR: Duration = Duration::from_secs(4 * 60);

    /// (Re)starts the alarm. A running alarm is replaced.
    pub fn start(&self) {
        let my_gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let generation = self.generation.clone();
        thread::spawn(move || {
            let stopped = || generation.load(Ordering::SeqCst) != my_gen;
            while !stopped() {
                let started = Instant::now();
                while !stopped() {
                    if let Err(e) = play_blocking(Sound::Alarm, &stopped) {
                        log::warn!("alarm sound failed: {e}");
                        thread::sleep(Duration::from_secs(1));
                    }
                    if started.elapsed() > Self::SOUND_FOR {
                        break;
                    }
                }
                let paused = Instant::now();
                while !stopped() && paused.elapsed() < Self::PAUSE_FOR {
                    thread::sleep(Duration::from_millis(100));
                }
            }
        });
    }

    pub fn stop(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plays ~1.5 s of each sound. Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn plays_sounds() {
        for s in [Sound::Alarm, Sound::CountdownEnd] {
            let t = Instant::now();
            play_blocking(s, &|| t.elapsed() > Duration::from_millis(1500)).unwrap();
        }
    }
}

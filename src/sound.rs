//! Sound playback for the alarm and the end of a countdown.
//!
//! The public API is the same on every platform:
//! - [`play_countdown_end`] plays the countdown sound once.
//! - [`AlarmPlayer`] beeps for one minute, pauses four minutes, and repeats
//!   until stopped. [`AlarmPlayer::poll`] must be called regularly (it drives
//!   the alarm on the web, and does nothing on the desktop).
//!
//! Desktop: background threads play the embedded WAV through `rodio` (with
//! the `native-audio` feature) or a system player (the default, so no ALSA
//! headers are needed to build).
//!
//! Web: no sound files. The tones are synthesized with the Web Audio API
//! (oscillators), which also keeps the WAVs out of the download. Browsers
//! only allow sound after the user has interacted with the page once.

/// How long the alarm beeps, then how long it waits before beeping again.
const ALARM_RING_MS: i64 = 60_000;
const ALARM_PAUSE_MS: i64 = 4 * 60_000;

#[cfg(not(target_arch = "wasm32"))]
pub use desktop::{AlarmPlayer, play_countdown_end};
#[cfg(target_arch = "wasm32")]
pub use web::{AlarmPlayer, play_countdown_end};

#[cfg(not(target_arch = "wasm32"))]
mod desktop {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    use super::{ALARM_PAUSE_MS, ALARM_RING_MS};

    /// The embedded sound files (desktop only).
    #[derive(Clone, Copy)]
    pub(super) enum Sound {
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
    }

    /// Plays `sound` once, blocking until it ends or `stop()` returns true.
    pub(super) fn play_blocking(sound: Sound, stop: &dyn Fn() -> bool) -> Result<(), String> {
        player::play(sound, stop)
    }

    /// Plays the countdown sound once in the background, retrying if the
    /// sound device is briefly busy (up to 50 attempts).
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

    /// Each `start` bumps the generation; a playing thread stops as soon as
    /// the generation is no longer the one it was started with.
    #[derive(Default)]
    pub struct AlarmPlayer {
        generation: Arc<AtomicU64>,
    }

    impl AlarmPlayer {
        /// (Re)starts the alarm. A running alarm is replaced.
        pub fn start(&mut self) {
            let my_gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
            let generation = self.generation.clone();
            let ring = Duration::from_millis(ALARM_RING_MS as u64);
            let pause = Duration::from_millis(ALARM_PAUSE_MS as u64);
            thread::spawn(move || {
                let stopped = || generation.load(Ordering::SeqCst) != my_gen;
                while !stopped() {
                    let started = Instant::now();
                    while !stopped() {
                        if let Err(e) = play_blocking(Sound::Alarm, &stopped) {
                            log::warn!("alarm sound failed: {e}");
                            thread::sleep(Duration::from_secs(1));
                        }
                        if started.elapsed() > ring {
                            break;
                        }
                    }
                    let paused = Instant::now();
                    while !stopped() && paused.elapsed() < pause {
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            });
        }

        pub fn stop(&mut self) {
            self.generation.fetch_add(1, Ordering::SeqCst);
        }

        /// Nothing to do: the alarm runs in its own thread.
        pub fn poll(&mut self, _now: i64) {}
    }

    #[cfg(feature = "native-audio")]
    mod player {
        use std::io::Cursor;
        use std::thread;
        use std::time::Duration;

        use super::Sound;

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
    mod player {
        use std::path::PathBuf;
        use std::process::{Command, Stdio};
        use std::thread;
        use std::time::Duration;

        use super::Sound;

        fn file_name(sound: Sound) -> &'static str {
            match sound {
                Sound::Alarm => "cronostack-alarm.wav",
                Sound::CountdownEnd => "cronostack-countdown-end.wav",
            }
        }

        /// The system players need a file, so the embedded WAV is written to
        /// the temp directory once.
        fn temp_file(sound: Sound) -> Result<PathBuf, String> {
            let path = std::env::temp_dir().join(file_name(sound));
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
                    continue; // player not installed, try the next one
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
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;

    use wasm_bindgen::JsValue;
    use web_sys::{AudioContext, OscillatorType};

    use super::{ALARM_PAUSE_MS, ALARM_RING_MS};

    /// While the alarm rings, a beep burst starts this often.
    const BURST_EVERY_MS: i64 = 2_000;

    thread_local! {
        /// One audio context for the whole page, created on first use.
        static CONTEXT: RefCell<Option<AudioContext>> = const { RefCell::new(None) };
    }

    /// Runs `play` with the page's audio context, creating it if needed.
    fn with_context(play: impl FnOnce(&AudioContext) -> Result<(), JsValue>) {
        CONTEXT.with(|cell| {
            let mut cell = cell.borrow_mut();
            if cell.is_none() {
                *cell = AudioContext::new()
                    .map_err(|e| log::warn!("no Web Audio: {e:?}"))
                    .ok();
            }
            if let Some(ctx) = cell.as_ref() {
                // Browsers keep a new context suspended until the user has
                // interacted with the page.
                let _ = ctx.resume();
                if let Err(e) = play(ctx) {
                    log::warn!("could not play sound: {e:?}");
                }
            }
        });
    }

    #[derive(Clone, Copy)]
    enum Envelope {
        /// On/off beep with tiny fades so it doesn't click.
        Beep,
        /// Starts loud and fades out, like a bell.
        Bell,
    }

    /// Schedules one tone: `freq_hz`, starting `start_s` seconds from now and
    /// lasting `length_s` seconds.
    fn tone(
        ctx: &AudioContext,
        wave: OscillatorType,
        envelope: Envelope,
        freq_hz: f32,
        start_s: f64,
        length_s: f64,
        volume: f32,
    ) -> Result<(), JsValue> {
        let oscillator = ctx.create_oscillator()?;
        oscillator.set_type(wave);
        oscillator.frequency().set_value(freq_hz);

        let gain = ctx.create_gain()?;
        let level = gain.gain();
        let t0 = ctx.current_time() + start_s;
        let t1 = t0 + length_s;
        level.set_value_at_time(0.0, t0)?;
        level.linear_ramp_to_value_at_time(volume, t0 + 0.005)?;
        match envelope {
            Envelope::Beep => {
                level.set_value_at_time(volume, t1 - 0.01)?;
                level.linear_ramp_to_value_at_time(0.0, t1)?;
            }
            Envelope::Bell => {
                level.exponential_ramp_to_value_at_time(0.0001, t1)?;
            }
        }

        oscillator.connect_with_audio_node(&gain)?;
        gain.connect_with_audio_node(&ctx.destination())?;
        oscillator.start_with_when(t0)?;
        oscillator.stop_with_when(t1)?;
        Ok(())
    }

    /// Pager-style burst: four short high beeps.
    fn alarm_burst() {
        with_context(|ctx| {
            for i in 0..4 {
                let start = f64::from(i) * 0.15;
                tone(
                    ctx,
                    OscillatorType::Square,
                    Envelope::Beep,
                    2_000.0,
                    start,
                    0.08,
                    0.12,
                )?;
            }
            Ok(())
        });
    }

    /// Three descending bell notes (C6, G5, C5).
    pub fn play_countdown_end() {
        with_context(|ctx| {
            for (i, freq) in [1_046.5, 784.0, 523.25].into_iter().enumerate() {
                let start = i as f64 * 0.3;
                tone(
                    ctx,
                    OscillatorType::Sine,
                    Envelope::Bell,
                    freq,
                    start,
                    0.7,
                    0.35,
                )?;
            }
            Ok(())
        });
    }

    enum Phase {
        Off,
        Ringing { until_ms: i64, next_burst_ms: i64 },
        Paused { until_ms: i64 },
    }

    /// No threads on the web: [`AlarmPlayer::poll`] moves between ringing
    /// (a burst every two seconds) and pausing.
    pub struct AlarmPlayer {
        phase: Phase,
    }

    impl Default for AlarmPlayer {
        fn default() -> Self {
            Self { phase: Phase::Off }
        }
    }

    impl AlarmPlayer {
        fn ring(&mut self, now: i64) {
            alarm_burst();
            self.phase = Phase::Ringing {
                until_ms: now + ALARM_RING_MS,
                next_burst_ms: now + BURST_EVERY_MS,
            };
        }

        pub fn start(&mut self) {
            self.ring(crate::clock::now_ms());
        }

        /// Stops future bursts; one already scheduled finishes (under 1 s).
        pub fn stop(&mut self) {
            self.phase = Phase::Off;
        }

        pub fn poll(&mut self, now: i64) {
            match self.phase {
                Phase::Ringing { until_ms, .. } if now >= until_ms => {
                    self.phase = Phase::Paused {
                        until_ms: now + ALARM_PAUSE_MS,
                    };
                }
                Phase::Ringing {
                    until_ms,
                    next_burst_ms,
                } if now >= next_burst_ms => {
                    alarm_burst();
                    self.phase = Phase::Ringing {
                        until_ms,
                        next_burst_ms: now + BURST_EVERY_MS,
                    };
                }
                Phase::Paused { until_ms } if now >= until_ms => self.ring(now),
                _ => {}
            }
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// Plays ~1.5 s of each sound. Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn plays_sounds() {
        for s in [desktop::Sound::Alarm, desktop::Sound::CountdownEnd] {
            let t = Instant::now();
            desktop::play_blocking(s, &|| t.elapsed() > Duration::from_millis(1500)).unwrap();
        }
    }
}

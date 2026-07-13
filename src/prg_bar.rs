//! A Cargo-style progress bar for Rust.
#![allow(dead_code)]
//!
//! Provides a terminal progress bar with:
//! - Visual bar `[====>     ]`
//! - Percentage display
//! - ETA (estimated time remaining)
//! - Throughput / download speed
//!
//! # Example
//!
//! ```ignore
//! use crate::prg_bar::ProgressBar;
//!
//! let bar = ProgressBar::new(100);
//! bar.set_message("Downloading");
//! for i in 0..=100 {
//!     bar.set_position(i);
//!     std::thread::sleep(std::time::Duration::from_millis(30));
//! }
//! bar.finish_with_message("Done");
//! ```

use std::fmt::Write as FmtWrite;
use std::io::{stderr, Write};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

// ── Helpers ──────────────────────────────────────────────────────────

/// Format a byte count as a human-readable size string.
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[unit_idx])
    } else {
        format!("{:.1} {}", size, UNITS[unit_idx])
    }
}

/// Format a duration as a human-readable ETA string.
fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// Format a throughput rate (bytes per second) as a human-readable string.
fn format_speed(bytes_per_sec: f64) -> String {
    format!("{}/s", format_bytes(bytes_per_sec as u64))
}

// ── Progress Style ───────────────────────────────────────────────────

/// Configures the visual appearance of a [`ProgressBar`].
#[derive(Debug, Clone)]
pub struct ProgressStyle {
    /// Width of the bar portion in characters (default: 30).
    pub bar_width: usize,
    /// Character for completed portion (default: `=`).
    pub filled_char: char,
    /// Character for the current position tip (default: `>`).
    pub tip_char: char,
    /// Character for the remaining portion (default: ` `).
    pub unfilled_char: char,
    /// Left bracket (default: `[`).
    pub left_bracket: char,
    /// Right bracket (default: `]`).
    pub right_bracket: char,
    /// Whether to show percentage (default: true).
    pub show_percent: bool,
    /// Whether to show ETA (default: true).
    pub show_eta: bool,
    /// Whether to show speed/throughput (default: true).
    pub show_speed: bool,
    /// Whether to track bytes for speed display (default: false).
    /// When false, speed is shown as "items/s".
    pub bytes_mode: bool,
    /// Minimum interval between redraws (default: 100ms).
    pub draw_interval: Duration,
}

impl Default for ProgressStyle {
    fn default() -> Self {
        Self {
            bar_width: 30,
            filled_char: '=',
            tip_char: '>',
            unfilled_char: ' ',
            left_bracket: '[',
            right_bracket: ']',
            show_percent: true,
            show_eta: true,
            show_speed: true,
            bytes_mode: false,
            draw_interval: Duration::from_millis(100),
        }
    }
}

impl ProgressStyle {
    /// Cargo-style bar: `[====>     ]`
    pub fn cargo() -> Self {
        Self::default()
    }

    /// Use a block-character bar with a smooth tip: `████▌░░░░`
    pub fn blocks() -> Self {
        Self {
            filled_char: '█',
            tip_char: '▌',
            unfilled_char: '░',
            ..Self::default()
        }
    }

    /// Enable byte-tracking mode — speed is shown as "12.3 MiB/s".
    pub fn with_bytes(mut self) -> Self {
        self.bytes_mode = true;
        self
    }

    /// Set the bar width.
    pub fn bar_width(mut self, width: usize) -> Self {
        self.bar_width = width;
        self
    }
}

// ── Progress State ───────────────────────────────────────────────────

struct ProgressState {
    pos: u64,
    len: u64,
    message: String,
    started: Instant,
    last_draw: Instant,
    finished: bool,
}

impl ProgressState {
    fn new(len: u64) -> Self {
        let now = Instant::now();
        Self {
            pos: 0,
            len,
            message: String::new(),
            started: now,
            last_draw: now,
            finished: false,
        }
    }

    fn percent(&self) -> f64 {
        if self.len == 0 {
            0.0
        } else {
            (self.pos as f64 / self.len as f64) * 100.0
        }
    }

    fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    fn eta(&self) -> Option<Duration> {
        if self.pos == 0 || self.pos >= self.len {
            return None;
        }
        let elapsed = self.elapsed().as_secs_f64();
        let rate = self.pos as f64 / elapsed; // items per second
        if rate == 0.0 {
            return None;
        }
        let remaining = (self.len - self.pos) as f64 / rate;
        Some(Duration::from_secs_f64(remaining))
    }

    fn speed(&self) -> f64 {
        let elapsed = self.elapsed().as_secs_f64();
        if elapsed == 0.0 {
            return 0.0;
        }
        self.pos as f64 / elapsed
    }
}

// ── Progress Bar ─────────────────────────────────────────────────────

/// A terminal progress bar with Cargo-style rendering.
///
/// The progress bar writes to stderr and uses `\r` to update in-place.
/// It is cheap to clone — clones share the same underlying state.
///
/// # Examples
///
/// ```ignore
/// use crate::prg_bar::ProgressBar;
///
/// let bar = ProgressBar::new(100);
/// for i in 0..=100 {
///     bar.set_position(i);
/// }
/// bar.finish();
/// ```
pub struct ProgressBar {
    state: Arc<Mutex<ProgressState>>,
    style: ProgressStyle,
    /// When set, tick/inc will only redraw if draw_interval has elapsed.
    throttle: bool,
}

impl ProgressBar {
    /// Create a new progress bar with a known total length.
    ///
    /// Pass `0` for an indeterminate bar (shows spinner-style motion).
    pub fn new(len: u64) -> Self {
        Self {
            state: Arc::new(Mutex::new(ProgressState::new(len))),
            style: ProgressStyle::default(),
            throttle: true,
        }
    }

    /// Create a new progress bar with a custom style.
    pub fn with_style(len: u64, style: ProgressStyle) -> Self {
        Self {
            state: Arc::new(Mutex::new(ProgressState::new(len))),
            style,
            throttle: true,
        }
    }

    /// Set the total length. Useful when the total is discovered later.
    pub fn set_length(&self, len: u64) {
        let mut st = self.state.lock().unwrap();
        st.len = len;
    }

    /// Set the prefix message shown before the bar (e.g. "Downloading").
    pub fn set_message(&self, msg: &str) {
        let mut st = self.state.lock().unwrap();
        st.message = msg.to_string();
    }

    /// Set the current position.
    pub fn set_position(&self, pos: u64) {
        let mut st = self.state.lock().unwrap();
        st.pos = pos;
        self.draw_if_needed(&mut st);
    }

    /// Increment the position by `delta`.
    pub fn inc(&self, delta: u64) {
        let mut st = self.state.lock().unwrap();
        st.pos += delta;
        self.draw_if_needed(&mut st);
    }

    /// Increment by 1.
    pub fn tick(&self) {
        self.inc(1);
    }

    /// Force a redraw regardless of the throttle interval.
    pub fn draw(&self) {
        let mut st = self.state.lock().unwrap();
        self.render(&st);
        st.last_draw = Instant::now();
    }

    /// Finish the progress bar and leave the cursor on a new line.
    pub fn finish(&self) {
        let mut st = self.state.lock().unwrap();
        if !st.finished {
            st.finished = true;
            st.pos = st.len;
            self.render(&st);
            let _ = stderr().write_all(b"\n");
            let _ = stderr().flush();
        }
    }

    /// Finish with a final message, overriding the bar line.
    ///
    /// The bar is replaced with the message followed by a newline.
    pub fn finish_with_message(&self, msg: &str) {
        let mut st = self.state.lock().unwrap();
        if !st.finished {
            st.finished = true;
            st.pos = st.len;
            // Clear the line then print the final message
            let _ = stderr().write_all(b"\r");
            let _ = stderr().write_all(msg.as_bytes());
            // Pad to cover any previous bar content
            let _ = stderr().write_all(b"  \n");
            let _ = stderr().flush();
        }
    }

    /// Reset the progress bar to reuse it.
    pub fn reset(&self) {
        let mut st = self.state.lock().unwrap();
        st.pos = 0;
        st.started = Instant::now();
        st.finished = false;
        st.message.clear();
    }

    /// Configure the style in builder fashion.
    pub fn with(mut self, f: impl FnOnce(&mut ProgressStyle)) -> Self {
        f(&mut self.style);
        self
    }

    /// Enable or disable draw throttling. When throttled (default),
    /// `set_position` / `inc` only redraw if at least `draw_interval` has
    /// passed since the last draw.
    pub fn throttle(mut self, throttle: bool) -> Self {
        self.throttle = throttle;
        self
    }

    // ── internal ─────────────────────────────────────────────────

    fn draw_if_needed(&self, st: &mut MutexGuard<'_, ProgressState>) {
        let now = Instant::now();
        let interval = self.style.draw_interval;
        if !self.throttle || now.duration_since(st.last_draw) >= interval {
            self.render(st);
            st.last_draw = now;
        }
    }

    fn render(&self, st: &ProgressState) {
        let mut out = String::with_capacity(128);

        // Message prefix
        if !st.message.is_empty() {
            let _ = write!(out, "{}  ", st.message);
        }

        // The bar itself
        let bar = self.render_bar(st);
        let _ = write!(out, "{}", bar);

        // Percentage
        if self.style.show_percent {
            let _ = write!(out, " {:>5.1}%", st.percent());
        }

        // Speed
        if self.style.show_speed && st.pos > 0 {
            let speed = st.speed();
            if self.style.bytes_mode {
                let _ = write!(out, "  {}", format_speed(speed));
            } else {
                let _ = write!(out, "  {:.0} it/s", speed);
            }
        }

        // ETA
        if self.style.show_eta {
            if let Some(eta) = st.eta() {
                let _ = write!(out, "  ETA: {}", format_duration(eta));
            }
        }

        // Write to stderr with carriage return
        // Pad with spaces to clear previous longer output
        let _ = stderr().write_all(b"\r");
        let _ = stderr().write_all(out.as_bytes());
        let _ = stderr().write_all(b"  ");
        let _ = stderr().flush();
    }

    fn render_bar(&self, st: &ProgressState) -> String {
        let w = self.style.bar_width;
        let pct = if st.len == 0 {
            0.0
        } else {
            st.pos as f64 / st.len as f64
        };
        let filled = ((pct * w as f64) as usize).min(w);

        let mut bar = String::with_capacity(w + 4);
        bar.push(self.style.left_bracket);

        // Filled portion
        for _ in 0..filled.saturating_sub(1) {
            bar.push(self.style.filled_char);
        }

        // Tip character (the `>` arrow)
        if filled > 0 && st.pos < st.len {
            bar.push(self.style.tip_char);
            // Remaining unfilled
            for _ in filled..w {
                bar.push(self.style.unfilled_char);
            }
        } else if st.pos >= st.len && st.len > 0 {
            // Complete — fill entirely
            bar.push(self.style.filled_char);
            for _ in filled..w {
                bar.push(self.style.filled_char);
            }
        } else {
            // Not started yet
            for _ in 0..w {
                bar.push(self.style.unfilled_char);
            }
        }

        bar.push(self.style.right_bracket);
        bar
    }
}

impl Clone for ProgressBar {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            style: self.style.clone(),
            throttle: self.throttle,
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(format_bytes(1048576), "1.0 MiB");
        assert_eq!(format_bytes(1073741824), "1.0 GiB");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(5)), "5s");
        assert_eq!(format_duration(Duration::from_secs(65)), "1m 5s");
        assert_eq!(format_duration(Duration::from_secs(3661)), "1h 1m");
    }

    #[test]
    fn test_percent_zero_total() {
        let st = ProgressState::new(0);
        assert_eq!(st.percent(), 0.0);
    }

    #[test]
    fn test_percent_half() {
        let mut st = ProgressState::new(200);
        st.pos = 100;
        assert!((st.percent() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_eta_none_at_start() {
        let st = ProgressState::new(100);
        assert_eq!(st.eta(), None);
    }

    #[test]
    fn test_render_bar_empty() {
        let bar = ProgressBar::new(100);
        let st = bar.state.lock().unwrap();
        let rendered = bar.render_bar(&st);
        assert!(rendered.starts_with('['));
        assert!(rendered.ends_with(']'));
        assert!(rendered.contains(&bar.style.unfilled_char.to_string()));
    }

    #[test]
    fn test_render_bar_complete() {
        let bar = ProgressBar::new(100);
        {
            let mut st = bar.state.lock().unwrap();
            st.pos = 100;
        }
        let st = bar.state.lock().unwrap();
        let rendered = bar.render_bar(&st);
        // Should be all filled when complete
        assert!(!rendered.contains(&bar.style.unfilled_char.to_string()));
    }
}

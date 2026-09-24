use colored::*;
use std::io::{stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

use crate::engine::arcade::{INVADER_FRAMES, PACMAN_FRAMES};

#[derive(Clone, Copy, Debug)]
pub enum ArcadeTheme {
    Invader,
    Pacman,
}

/// 8-Bit Arcade Terminal Animated Loader for LLM and tool operations.
/// Cycles through 8-bit pixel frames and neon CRT colors, clearing upon completion.
pub struct Spinner {
    running: Arc<AtomicBool>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl Spinner {
    /// Starts an 8-bit arcade spinner with the default Invader animation.
    pub fn start(message: impl Into<String>) -> Self {
        Self::start_with_theme(ArcadeTheme::Invader, message)
    }

    /// Starts an 8-bit arcade spinner with a specific theme.
    pub fn start_with_theme(theme: ArcadeTheme, message: impl Into<String>) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let message = message.into();

        let handle = tokio::spawn(async move {
            let frames = match theme {
                ArcadeTheme::Invader => INVADER_FRAMES,
                ArcadeTheme::Pacman => PACMAN_FRAMES,
            };

            let start_time = std::time::Instant::now();
            let mut i = 0;
            while running_clone.load(Ordering::Relaxed) {
                let raw_frame = frames[i % frames.len()];
                // 8-bit neon color cycling
                let colored_frame = match (i / 2) % 4 {
                    0 => raw_frame.cyan().bold(),
                    1 => raw_frame.yellow().bold(),
                    2 => raw_frame.magenta().bold(),
                    _ => raw_frame.green().bold(),
                };

                let elapsed = start_time.elapsed().as_secs();
                let status_suffix = if elapsed >= 45 {
                    format!("({}s) [buffering... still working]", elapsed).red().bold()
                } else if elapsed >= 15 {
                    format!("({}s) [buffering...]", elapsed).yellow().bold()
                } else if elapsed >= 4 {
                    format!("({}s)", elapsed).dimmed()
                } else {
                    "".normal()
                };

                print!("\r  {} {} {} ", colored_frame, message.white(), status_suffix);
                let _ = stdout().flush();
                i += 1;
                sleep(Duration::from_millis(90)).await;
            }
            // Clear the spinner line
            print!("\r\x1B[2K");
            let _ = stdout().flush();
        });

        Self {
            running,
            handle: Some(handle),
        }
    }

    /// Stops the spinner and erases the line cleanly.
    pub fn stop(&mut self) {
        if let Some(handle) = self.handle.take() {
            self.running.store(false, Ordering::Relaxed);
            handle.abort();
            print!("\r\x1B[2K");
            let _ = stdout().flush();
        }
    }

    /// Stops the spinner and prints an 8-bit retro status message (e.g. "★ [STAGE CLEAR] ...").
    pub fn stop_with_message(&mut self, status_icon: &str, final_message: &str) {
        self.stop();
        println!("  {} {}", status_icon.bold().yellow(), final_message.bold().white());
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop();
    }
}

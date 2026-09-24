use colored::*;
use std::io::{stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Terminal animated loading spinner for async LLM and tool operations.
/// Runs in a background Tokio task and automatically clears its line upon completion.
pub struct Spinner {
    running: Arc<AtomicBool>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl Spinner {
    /// Starts a spinner with the given message (e.g. "Thinking... (gpt-4o)")
    pub fn start(message: impl Into<String>) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let message = message.into();

        let handle = tokio::spawn(async move {
            let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let mut i = 0;
            while running_clone.load(Ordering::Relaxed) {
                let frame = frames[i % frames.len()].yellow();
                print!("\r  {} {} ", frame, message.dimmed());
                let _ = stdout().flush();
                i += 1;
                sleep(Duration::from_millis(80)).await;
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

    /// Stops the spinner and erases the spinner line from the terminal.
    pub fn stop(&mut self) {
        if let Some(handle) = self.handle.take() {
            self.running.store(false, Ordering::Relaxed);
            handle.abort();
            print!("\r\x1B[2K");
            let _ = stdout().flush();
        }
    }

    /// Stops the spinner and prints a final status line (e.g. "✓ Thinking complete (0.8s)")
    pub fn stop_with_message(&mut self, status_icon: &str, final_message: &str) {
        self.stop();
        println!("  {} {}", status_icon, final_message);
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop();
    }
}

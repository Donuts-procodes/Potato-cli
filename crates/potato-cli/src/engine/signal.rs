use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::info;

/// Global shutdown signal shared across the application.
/// When set to `true`, the super loop should gracefully exit at the next turn boundary.
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Returns `true` if a shutdown signal (Ctrl+C) has been received.
pub fn is_shutdown_requested() -> bool {
    SHUTDOWN_REQUESTED.load(Ordering::Relaxed)
}

/// Installs the Ctrl+C signal handler.
/// First Ctrl+C sets the graceful shutdown flag.
/// Second Ctrl+C forces immediate process exit.
pub fn install_signal_handler() -> Result<()> {
    let first_signal = Arc::new(AtomicBool::new(false));
    let first_signal_clone = first_signal.clone();

    ctrlc::set_handler(move || {
        if first_signal_clone.swap(true, Ordering::SeqCst) {
            // Second Ctrl+C — force exit
            eprintln!("\n⚡ Force shutdown — exiting immediately");
            std::process::exit(130);
        } else {
            // First Ctrl+C — graceful shutdown
            SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
            eprintln!("\n🛑 Shutdown requested — finishing current turn, then exiting cleanly...");
            eprintln!("   Press Ctrl+C again to force exit immediately.");
        }
    })?;

    info!("Signal handler installed (Ctrl+C for graceful shutdown)");
    Ok(())
}

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::StashError;

pub const CLIPBOARD_TTL: Duration = Duration::from_secs(30);

/// TTL used by `pwstash copy`. Defaults to [`CLIPBOARD_TTL`].
/// `PWSTASH_CLIPBOARD_TTL_MS` shortens the wait for tests.
pub fn clipboard_ttl() -> Duration {
    std::env::var("PWSTASH_CLIPBOARD_TTL_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .map(Duration::from_millis)
        .filter(|d| *d > Duration::ZERO)
        .unwrap_or(CLIPBOARD_TTL)
}

pub fn copy_text(text: &str) -> Result<(), StashError> {
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text(text.to_string()))
        .map_err(|err| StashError::Clipboard(err.to_string()))
}

pub fn clear() -> Result<(), StashError> {
    copy_text("")
}

pub fn wait_then_clear() -> Result<(), StashError> {
    let stop = Arc::new(AtomicBool::new(false));
    let _ = ctrlc::set_handler({
        let stop = Arc::clone(&stop);
        move || stop.store(true, Ordering::SeqCst)
    });

    let deadline = Instant::now() + clipboard_ttl();
    while Instant::now() < deadline && !stop.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(50));
    }

    clear()
}

pub fn copy_then_clear_blocking(text: &str) -> Result<(), StashError> {
    copy_text(text)?;
    wait_then_clear()
}

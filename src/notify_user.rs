//! Best-effort desktop notifications via `notify-send` (libnotify).

use std::process::Command;

/// Shows a non-blocking notification if `notify-send` is available.
pub fn spawn_or_window_failure(summary: &str, body: &str) {
    let _ = Command::new("notify-send")
        .args(["-a", "niri-session-manage", summary, body])
        .status();
}

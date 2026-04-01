//! Heuristics for `/proc/*/cmdline` values that must not be re-run on `--load`.

use std::path::Path;

fn basename_arg0(arg0: &str) -> &str {
    Path::new(arg0)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(arg0)
}

/// If `Some`, the argv must not be spawned: it is tied to the current compositor session or is an
/// internal bridge (e.g. [xwayland-satellite](https://github.com/niri-wm/niri)) rather than the
/// user's application.
pub fn unrestorable_reason(cmd: &[String]) -> Option<&'static str> {
    if cmd.is_empty() {
        return None;
    }
    if cmd.iter().any(|a| a == "-listenfd") {
        return Some(
            "argv contains -listenfd (ephemeral fds from the current session; not portable)",
        );
    }
    if basename_arg0(&cmd[0]) == "xwayland-satellite" {
        return Some(
            "argv is xwayland-satellite (spawned by niri for X11 clients; replace with the real app, e.g. google-chrome or flatpak run …)",
        );
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xwayland_satellite_is_unrestorable() {
        let cmd = vec![
            "xwayland-satellite".into(),
            ":1".into(),
            "-listenfd".into(),
            "145".into(),
        ];
        assert!(unrestorable_reason(&cmd).is_some());
    }

    #[test]
    fn listenfd_anywhere_flags() {
        let cmd = vec!["foo".into(), "-listenfd".into(), "3".into()];
        assert!(unrestorable_reason(&cmd).is_some());
    }

    #[test]
    fn normal_command_ok() {
        let cmd = vec!["foot".into()];
        assert!(unrestorable_reason(&cmd).is_none());
    }
}

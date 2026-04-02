//! Heuristics for `/proc/*/cmdline` values that must not be re-run on `--load`.

/// If `Some`, the argv must not be spawned without a `[[launch]]` override: ephemeral session
/// state (e.g. `-listenfd`). Program-specific bridges (e.g. `xwayland-satellite`) are configured
/// via `[[launch]].resolve` in `niri-session.conf`.
pub fn unrestorable_reason(cmd: &[String]) -> Option<&'static str> {
    if cmd.is_empty() {
        return None;
    }
    if cmd.iter().any(|a| a == "-listenfd") {
        return Some(
            "argv contains -listenfd (ephemeral fds from the current session; not portable)",
        );
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listenfd_with_xwayland_argv_is_unrestorable() {
        let cmd = vec![
            "xwayland-satellite".into(),
            ":1".into(),
            "-listenfd".into(),
            "145".into(),
        ];
        assert!(unrestorable_reason(&cmd).is_some());
    }

    #[test]
    fn xwayland_basename_without_listenfd_is_restorable_by_policy() {
        let cmd = vec!["xwayland-satellite".into(), ":1".into()];
        assert!(unrestorable_reason(&cmd).is_none());
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

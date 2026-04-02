//! Smoke tests for the `niri-session` binary (help, version, basic error paths).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Path to the freshly built `niri-session` binary.
///
/// Cargo normally sets `CARGO_BIN_EXE_niri_session` when running integration tests; if it is
/// missing (unusual environments), we fall back to `target/<profile>/niri-session` under the
/// manifest dir or `CARGO_TARGET_DIR`.
fn bin() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_niri_session") {
        let path = PathBuf::from(p);
        assert!(
            path.exists(),
            "CARGO_BIN_EXE_niri_session points at missing file: {}",
            path.display()
        );
        return path;
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let path = target_dir.join(profile).join("niri-session");
    assert!(
        path.exists(),
        "niri-session binary not found at {}. Run `cargo build` or `cargo test` from the crate root first.",
        path.display()
    );
    path
}

#[test]
fn help_exits_zero_and_lists_actions() {
    let o = Command::new(bin())
        .arg("--help")
        .output()
        .expect("spawn niri-session --help");
    assert!(
        o.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&o.stderr)
    );
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        out.contains("-d") && out.contains("--debug"),
        "help should mention -d and --debug:\n{out}"
    );
    assert!(out.contains("--save"), "help should mention --save:\n{out}");
    assert!(out.contains("--load"), "help should mention --load:\n{out}");
    assert!(
        out.contains("ipc-settle-ms") || out.contains("ipc_settle"),
        "help should mention ipc-settle-ms:\n{out}"
    );
    assert!(
        out.contains("--config") || out.contains("config"),
        "help should mention --config:\n{out}"
    );
    assert!(
        out.contains("spawn-start-delay") && out.contains("no-notify"),
        "help should mention spawn-start-delay and notify flags:\n{out}"
    );
    assert!(
        out.contains("print-niri-hotkey-overlay-bind"),
        "help should mention print-niri-hotkey-overlay-bind:\n{out}"
    );
    assert!(
        out.contains("graceful-shutdown") && out.contains("load-last"),
        "help should mention graceful-shutdown and load-last:\n{out}"
    );
}

#[test]
fn print_niri_hotkey_overlay_bind_exits_zero() {
    let o = Command::new(bin())
        .arg("--print-niri-hotkey-overlay-bind")
        .output()
        .expect("spawn print bind");
    assert!(
        o.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&o.stderr)
    );
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        out.contains("show-hotkey-overlay") && out.contains("Mod+Shift+Slash"),
        "expected KDL bind line, got:\n{out}"
    );
}

#[test]
fn version_exits_zero() {
    let o = Command::new(bin())
        .arg("--version")
        .output()
        .expect("spawn niri-session --version");
    assert!(o.status.success());
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        out.contains("niri-session"),
        "expected version string, got:\n{out}"
    );
}

#[test]
fn no_save_or_load_fails() {
    let o = Command::new(bin())
        .output()
        .expect("spawn niri-session with no args");
    assert!(
        !o.status.success(),
        "expected failure without --save/--load"
    );
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(
        err.contains("save") && err.contains("load") && err.contains("graceful-shutdown"),
        "stderr should hint at modes, got:\n{err}"
    );
}

#[test]
fn save_without_niri_socket_fails_cleanly() {
    let tmp = std::env::temp_dir().join("niri-session-test-save.json");
    let o = Command::new(bin())
        .env_remove("NIRI_SOCKET")
        .arg("--save")
        .arg(&tmp)
        .output()
        .expect("spawn niri-session --save");
    assert!(!o.status.success(), "save without NIRI_SOCKET should fail");
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(
        err.contains("NIRI_SOCKET"),
        "stderr should mention NIRI_SOCKET, got:\n{err}"
    );
}

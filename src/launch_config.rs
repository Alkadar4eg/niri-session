//! TOML config: map `app_id` / `title` → launch `command` for windows whose saved argv is not portable.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::cmdline_policy;
use crate::error::{Error, Result};
use crate::session::WindowEntry;

/// Default timing values when neither CLI nor `[load]` sets them.
pub const DEFAULT_SPAWN_POLL_MS: u64 = 50;
/// How long to wait for a new window to appear after `spawn` (same PID in IPC).
pub const DEFAULT_SPAWN_TIMEOUT_MS: u64 = 2000;
pub const DEFAULT_IPC_SETTLE_MS: u64 = 80;
pub const DEFAULT_SPAWN_START_DELAY_MS: u64 = 0;

/// Parsed `niri-session.conf`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LaunchConfig {
    /// Defaults for `--load` (timings, notifications). Optional; see [LoadSettings].
    #[serde(default)]
    pub load: LoadSettings,
    /// First matching rule wins. Put more specific rules (`app_id` + `title_contains`) first.
    #[serde(default)]
    pub launch: Vec<LaunchRule>,
}

/// Optional defaults for session restore, TOML table `[load]`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LoadSettings {
    pub spawn_poll_ms: Option<u64>,
    pub spawn_timeout_ms: Option<u64>,
    pub ipc_settle_ms: Option<u64>,
    pub spawn_start_delay_ms: Option<u64>,
    /// If `false`, do not call `notify-send` on spawn/window timeout failures.
    pub notify_on_spawn_failure: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LaunchRule {
    /// If set, must match `WindowEntry.app_id` exactly.
    pub app_id: Option<String>,
    /// If set, `WindowEntry.title` must contain this substring.
    pub title_contains: Option<String>,
    /// argv to run instead of the saved `command` when this rule matches.
    pub command: Vec<String>,
}

/// Default: `~/.config/niri/niri-session.conf` (via `dirs` / `$HOME/.config`).
pub fn default_config_path() -> PathBuf {
    dirs::config_dir()
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|p| p.join("niri").join("niri-session.conf"))
        .unwrap_or_else(|| PathBuf::from("niri-session.conf"))
}

/// Load launch overrides.
///
/// - `explicit_path: None` — read [`default_config_path`] if the file exists; otherwise empty rules.
/// - `explicit_path: Some(p)` — file **must** exist and parse.
pub fn load(explicit_path: Option<&Path>) -> Result<LaunchConfig> {
    match explicit_path {
        None => {
            let path = default_config_path();
            if !path.exists() {
                return Ok(LaunchConfig::default());
            }
            parse_file(&path)
        }
        Some(p) => {
            if !p.exists() {
                return Err(Error::ConfigNotFound(p.to_path_buf()));
            }
            parse_file(p)
        }
    }
}

fn parse_file(path: &Path) -> Result<LaunchConfig> {
    let s = fs::read_to_string(path).map_err(Error::Io)?;
    let cfg: LaunchConfig = toml::from_str(&s).map_err(|e| Error::ConfigToml {
        path: path.to_path_buf(),
        msg: e.to_string(),
    })?;
    validate(&cfg, path)?;
    Ok(cfg)
}

fn validate(cfg: &LaunchConfig, path: &Path) -> Result<()> {
    for (i, rule) in cfg.launch.iter().enumerate() {
        if rule.app_id.is_none() && rule.title_contains.is_none() {
            return Err(Error::ConfigInvalid {
                path: path.to_path_buf(),
                msg: format!(
                    "launch[{i}]: set at least one of app_id or title_contains"
                ),
            });
        }
        if rule.command.is_empty() {
            return Err(Error::ConfigInvalid {
                path: path.to_path_buf(),
                msg: format!("launch[{i}]: command must not be empty"),
            });
        }
    }
    Ok(())
}

fn rule_matches(rule: &LaunchRule, win: &WindowEntry) -> bool {
    if let Some(ref expected) = rule.app_id {
        if win.app_id.as_deref() != Some(expected.as_str()) {
            return false;
        }
    }
    if let Some(ref sub) = rule.title_contains {
        let title = win.title.as_deref().unwrap_or("");
        if !title.contains(sub.as_str()) {
            return false;
        }
    }
    true
}

/// Returns argv to `spawn`: either the saved cmdline or an override for non-portable lines.
pub fn resolve_spawn_command(win: &WindowEntry, cfg: &LaunchConfig) -> Result<Vec<String>> {
    if cmdline_policy::unrestorable_reason(&win.command).is_none() {
        return Ok(win.command.clone());
    }
    for rule in &cfg.launch {
        if rule_matches(rule, win) {
            return Ok(rule.command.clone());
        }
    }
    Err(Error::MissingLaunchOverride {
        cmd: win.command.clone(),
        app_id: win.app_id.clone(),
        title: win.title.clone(),
    })
}

/// Notify user on spawn failures: CLI `--no-notify-on-spawn-failure` wins, then env
/// `NIRI_SESSION_NOTIFY_ON_SPAWN_FAILURE`, then `[load].notify_on_spawn_failure`, then `true`.
pub fn merged_notify_on_failure(no_notify_cli: bool, cfg: &LaunchConfig) -> bool {
    if no_notify_cli {
        return false;
    }
    if let Ok(v) = std::env::var("NIRI_SESSION_NOTIFY_ON_SPAWN_FAILURE") {
        let lower = v.to_ascii_lowercase();
        if matches!(lower.as_str(), "0" | "false" | "no" | "off") {
            return false;
        }
        if matches!(lower.as_str(), "1" | "true" | "yes" | "on") {
            return true;
        }
    }
    cfg.load.notify_on_spawn_failure.unwrap_or(true)
}

#[cfg(test)]
mod notify_merge_tests {
    use super::*;

    #[test]
    fn cli_no_notify_wins() {
        let cfg = LaunchConfig {
            load: LoadSettings {
                notify_on_spawn_failure: Some(true),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(!merged_notify_on_failure(true, &cfg));
    }

    #[test]
    fn config_can_disable() {
        let cfg = LaunchConfig {
            load: LoadSettings {
                notify_on_spawn_failure: Some(false),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(!merged_notify_on_failure(false, &cfg));
    }

    #[test]
    fn default_notify_true() {
        let cfg = LaunchConfig::default();
        assert!(merged_notify_on_failure(false, &cfg));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(app_id: Option<&str>, title: Option<&str>, cmd: &[&str]) -> WindowEntry {
        WindowEntry {
            command: cmd.iter().map(|s| (*s).to_string()).collect(),
            app_id: app_id.map(String::from),
            title: title.map(String::from),
            output: "O".into(),
            workspace_idx: 1,
            column: 1,
            tile: 1,
            is_floating: false,
        }
    }

    #[test]
    fn override_by_app_id_only() {
        let cfg = LaunchConfig {
            load: LoadSettings::default(),
            launch: vec![LaunchRule {
                app_id: Some("Google-chrome".into()),
                title_contains: None,
                command: vec!["google-chrome-stable".into()],
            }],
        };
        let w = win(
            Some("Google-chrome"),
            None,
            &["xwayland-satellite", ":1", "-listenfd", "1"],
        );
        let argv = resolve_spawn_command(&w, &cfg).expect("resolve");
        assert_eq!(argv, vec!["google-chrome-stable"]);
    }

    #[test]
    fn more_specific_rule_first() {
        let cfg = LaunchConfig {
            load: LoadSettings::default(),
            launch: vec![
                LaunchRule {
                    app_id: Some("Google-chrome".into()),
                    title_contains: Some("VK".into()),
                    command: vec!["chrome-vk".into()],
                },
                LaunchRule {
                    app_id: Some("Google-chrome".into()),
                    title_contains: None,
                    command: vec!["chrome-generic".into()],
                },
            ],
        };
        let w = win(
            Some("Google-chrome"),
            Some("VK Messenger"),
            &["xwayland-satellite"],
        );
        let argv = resolve_spawn_command(&w, &cfg).expect("resolve");
        assert_eq!(argv, vec!["chrome-vk"]);
    }

    #[test]
    fn portable_cmd_ignores_config() {
        let cfg = LaunchConfig {
            load: LoadSettings::default(),
            launch: vec![LaunchRule {
                app_id: Some("foot".into()),
                title_contains: None,
                command: vec!["wrong".into()],
            }],
        };
        let w = win(Some("foot"), None, &["foot"]);
        let argv = resolve_spawn_command(&w, &cfg).expect("resolve");
        assert_eq!(argv, vec!["foot"]);
    }

    #[test]
    fn no_override_errors() {
        let cfg = LaunchConfig::default();
        let w = win(
            Some("Google-chrome"),
            None,
            &["xwayland-satellite", "-listenfd", "1"],
        );
        assert!(resolve_spawn_command(&w, &cfg).is_err());
    }
}

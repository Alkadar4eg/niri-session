//! TOML config: map `app_id` / `title` → launch `command` for windows whose saved argv is not portable.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer};

use crate::chrome_pwa;
use crate::cmdline_policy;
use crate::debug_log::DebugLog;
use crate::error::{Error, Result};
use crate::session::WindowEntry;

pub const DEFAULT_IPC_SETTLE_MS: u64 = 80;
pub const DEFAULT_SPAWN_START_DELAY_MS: u64 = 0;
/// Максимальное время ожидания появления нового окна в списке niri после `spawn` (режим с ожиданием).
pub const DEFAULT_SPAWN_DEADLINE_MS: u64 = 10_000;

/// Parsed `niri-session.conf`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LaunchConfig {
    /// Default path for `niri-session --save` without argument. Optional; see [SessionSettings].
    #[serde(default)]
    pub session: SessionSettings,
    /// Defaults for `--load` (timings, notifications). Optional; see [LoadSettings].
    #[serde(default)]
    pub load: LoadSettings,
    /// First matching rule wins. Put more specific rules (`app_id` + `title_contains`) first.
    #[serde(default)]
    pub launch: Vec<LaunchRule>,
}

/// Optional defaults for session file location, TOML table `[session]`.
#[derive(Debug, Clone, Deserialize)]
pub struct SessionSettings {
    /// Directory for session JSON files. Used when `--save` / `--load` get a bare filename, or when
    /// no path is given (then `session.json` in this directory). Leading `~/` is expanded.
    #[serde(default)]
    pub default_session_dir: Option<String>,
    /// Basename (or path) for `--graceful-shutdown` save and `--load-last`. Same resolution as `--save`/`--load`.
    #[serde(default = "default_graceful_shutdown_name")]
    pub graceful_shutdown_name: String,
}

fn default_graceful_shutdown_name() -> String {
    "last".to_string()
}

impl Default for SessionSettings {
    fn default() -> Self {
        Self {
            default_session_dir: None,
            graceful_shutdown_name: default_graceful_shutdown_name(),
        }
    }
}

/// Resolved path for `[session].graceful_shutdown_name` (`--graceful-shutdown`, `--load-last`).
pub fn graceful_shutdown_session_path(cfg: &LaunchConfig) -> PathBuf {
    resolve_session_file_path(Path::new(&cfg.session.graceful_shutdown_name), cfg)
}

/// Default basename for `niri-session --save` / `--load` with no PATH argument.
pub const DEFAULT_SESSION_BASENAME: &str = "session.json";

/// Expands a leading `~/` in a path string to the home directory.
pub fn expand_path_str(s: &str) -> PathBuf {
    let s = s.trim();
    if s.is_empty() {
        return PathBuf::new();
    }
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(h) = dirs::home_dir() {
            return h.join(rest);
        }
    }
    PathBuf::from(s)
}

/// Fallback when env and config do not set a directory: `~/.config/niri-session/sessions`.
pub fn default_session_dir() -> PathBuf {
    dirs::config_dir()
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|p| p.join("niri-session").join("sessions"))
        .unwrap_or_else(|| PathBuf::from("sessions"))
}

/// Effective session directory: `NIRI_SESSION_DIR` → `[session].default_session_dir` → built-in default.
pub fn merged_default_session_dir(cfg: &LaunchConfig) -> PathBuf {
    if let Some(os) = std::env::var_os("NIRI_SESSION_DIR") {
        let p = PathBuf::from(os);
        if !p.as_os_str().is_empty() {
            return p;
        }
    }
    cfg.session
        .default_session_dir
        .as_ref()
        .map(|s| expand_path_str(s))
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(default_session_dir)
}

/// True if `path` is a single path segment (e.g. `foo.json`), not `a/b` or `/abs`.
fn is_single_path_segment(path: &Path) -> bool {
    let mut c = path.components();
    matches!(
        (c.next(), c.next()),
        (Some(std::path::Component::Normal(_)), None)
    )
}

/// Resolves `--save` / `--load` argument: absolute paths as-is; a single filename joins
/// [`merged_default_session_dir`]; other relative paths are resolved from the current directory.
pub fn resolve_session_file_path(user: &Path, cfg: &LaunchConfig) -> PathBuf {
    if user.as_os_str().is_empty() {
        return merged_default_session_dir(cfg).join(DEFAULT_SESSION_BASENAME);
    }
    if user.is_absolute() {
        return user.to_path_buf();
    }
    if is_single_path_segment(user) {
        merged_default_session_dir(cfg).join(user)
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(user))
            .unwrap_or_else(|_| user.to_path_buf())
    }
}

/// Optional defaults for session restore, TOML table `[load]`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LoadSettings {
    pub ipc_settle_ms: Option<u64>,
    pub spawn_start_delay_ms: Option<u64>,
    /// If `true`, do not wait for a new window id after each spawn (fire-and-forget).
    pub no_await: Option<bool>,
    /// Max time in ms to wait for a new window after spawn when awaiting is enabled.
    pub spawn_deadline: Option<u64>,
    /// If `false`, do not call `notify-send` on launch failures (resolve/spawn/empty command).
    pub notify_on_spawn_failure: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LaunchRule {
    /// If set, must match `WindowEntry.app_id` exactly.
    pub app_id: Option<String>,
    /// If set, `WindowEntry.title` must contain this substring.
    pub title_contains: Option<String>,
    /// If set, this rule applies only when `argv[0]` basename equals this string (e.g. `xwayland-satellite`),
    /// or use `resolve = "-listenfd"` when argv contains `-listenfd`. If omitted, the rule applies only
    /// when the built-in unrestorable check (`-listenfd`) matches.
    pub resolve: Option<String>,
    /// argv to run instead of the saved `command` when this rule matches.
    /// TOML: массив строк или одна строка с аргументами (`shlex`, кавычки как в shell).
    #[serde(deserialize_with = "deserialize_launch_command")]
    pub command: Vec<String>,
}

fn deserialize_launch_command<'de, D>(deserializer: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum RawCommand {
        Str(String),
        Arr(Vec<String>),
    }

    match RawCommand::deserialize(deserializer)? {
        RawCommand::Str(s) => {
            let s = s.trim();
            if s.is_empty() {
                return Err(D::Error::custom("command must not be empty"));
            }
            shlex::split(s).ok_or_else(|| D::Error::custom("command: unclosed quote"))
        }
        RawCommand::Arr(v) => Ok(v),
    }
}

/// Default: `~/.config/niri-session/niri-session.conf` (via `dirs` / `$HOME/.config`).
pub fn default_config_path() -> PathBuf {
    dirs::config_dir()
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|p| p.join("niri-session").join("niri-session.conf"))
        .unwrap_or_else(|| PathBuf::from("niri-session.conf"))
}

/// Load launch overrides.
///
/// - `explicit_path: None` — read [`default_config_path`] if the file exists; otherwise empty rules.
/// - `explicit_path: Some(p)` — file **must** exist and parse.
pub fn load(explicit_path: Option<&Path>, debug: DebugLog) -> Result<LaunchConfig> {
    match explicit_path {
        None => {
            let path = default_config_path();
            debug.log(format!(
                "config: default path={} exists={}",
                path.display(),
                path.exists()
            ));
            if !path.exists() {
                debug.log("config: no default file, empty [[launch]]");
                return Ok(LaunchConfig::default());
            }
            let cfg = parse_file(&path)?;
            debug.log(format!(
                "config: loaded {} [[launch]] from {}",
                cfg.launch.len(),
                path.display()
            ));
            Ok(cfg)
        }
        Some(p) => {
            debug.log(format!("config: explicit path={}", p.display()));
            if !p.exists() {
                return Err(Error::ConfigNotFound(p.to_path_buf()));
            }
            let cfg = parse_file(p)?;
            debug.log(format!(
                "config: loaded {} [[launch]] rules",
                cfg.launch.len()
            ));
            Ok(cfg)
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
                msg: format!("launch[{i}]: set at least one of app_id or title_contains"),
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

/// `resolve` matches `argv[0]` basename, or special `-listenfd` / `listenfd` when `-listenfd` appears in argv.
fn command_matches_resolve(cmd: &[String], resolve: &str) -> bool {
    if resolve == "-listenfd" || resolve.eq_ignore_ascii_case("listenfd") {
        return cmd.iter().any(|a| a == "-listenfd");
    }
    if cmd.is_empty() {
        return false;
    }
    let arg0 = &cmd[0];
    let base = Path::new(arg0)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(arg0.as_str());
    base == resolve
}

/// If `command` is a single string that looks like a full cmdline (spaces), split like a shell (`shlex`).
/// Covers odd `/proc/*/cmdline` or legacy JSON with one concatenated argv entry.
fn normalized_argv(cmd: &[String]) -> Vec<String> {
    if cmd.len() == 1 {
        let s = &cmd[0];
        if s.contains(' ') {
            if let Some(parts) = shlex::split(s) {
                if parts.len() > 1 {
                    return parts;
                }
            }
        }
    }
    cmd.to_vec()
}

fn rule_applies(rule: &LaunchRule, win: &WindowEntry) -> bool {
    if !rule_matches(rule, win) {
        return false;
    }
    match &rule.resolve {
        Some(r) => {
            command_matches_resolve(&win.command, r) || rule.title_contains.is_some()
        }
        None => {
            cmdline_policy::unrestorable_reason(&win.command).is_some()
                || rule.title_contains.is_some()
        }
    }
}

/// Returns argv to `spawn`: either the saved cmdline or an override from `[[launch]]`.
pub fn resolve_spawn_command(win: &WindowEntry, cfg: &LaunchConfig) -> Result<Vec<String>> {
    let mut win = win.clone();
    win.command = normalized_argv(&win.command);
    chrome_pwa::align_chrome_pwa_argv(win.app_id.as_deref(), &mut win.command);

    if !cfg.launch.is_empty() {
        for rule in &cfg.launch {
            if rule_applies(rule, &win) {
                return Ok(rule.command.clone());
            }
        }
    }
    if cmdline_policy::unrestorable_reason(&win.command).is_some() {
        return Err(Error::MissingLaunchOverride {
            cmd: win.command.clone(),
            app_id: win.app_id.clone(),
            title: win.title.clone(),
        });
    }
    Ok(win.command)
}

/// Notify user on launch failures: CLI `--no-notify-on-spawn-failure` wins, then env
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
    use std::path::{Path, PathBuf};

    #[test]
    fn merged_default_session_dir_from_config() {
        let saved = std::env::var_os("NIRI_SESSION_DIR");
        std::env::remove_var("NIRI_SESSION_DIR");
        let cfg = LaunchConfig {
            session: SessionSettings {
                default_session_dir: Some("/var/tmp/sessions".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            merged_default_session_dir(&cfg),
            PathBuf::from("/var/tmp/sessions")
        );
        if let Some(s) = saved {
            std::env::set_var("NIRI_SESSION_DIR", s);
        }
    }

    #[test]
    fn merged_default_session_dir_builtin_without_config() {
        let saved = std::env::var_os("NIRI_SESSION_DIR");
        std::env::remove_var("NIRI_SESSION_DIR");
        assert_eq!(
            merged_default_session_dir(&LaunchConfig::default()),
            default_session_dir()
        );
        if let Some(s) = saved {
            std::env::set_var("NIRI_SESSION_DIR", s);
        }
    }

    #[test]
    fn expand_path_tilde() {
        let h = dirs::home_dir().expect("home");
        assert_eq!(expand_path_str("~/foo/bar.json"), h.join("foo/bar.json"));
    }

    #[test]
    fn resolve_single_name_uses_session_dir() {
        let saved = std::env::var_os("NIRI_SESSION_DIR");
        std::env::remove_var("NIRI_SESSION_DIR");
        let cfg = LaunchConfig {
            session: SessionSettings {
                default_session_dir: Some("/data/s".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            resolve_session_file_path(Path::new("work.json"), &cfg),
            PathBuf::from("/data/s/work.json")
        );
        if let Some(s) = saved {
            std::env::set_var("NIRI_SESSION_DIR", s);
        }
    }

    #[test]
    fn resolve_absolute_unchanged() {
        let cfg = LaunchConfig::default();
        assert_eq!(
            resolve_session_file_path(Path::new("/tmp/x.json"), &cfg),
            PathBuf::from("/tmp/x.json")
        );
    }

    #[test]
    fn graceful_shutdown_path_default_name() {
        let saved = std::env::var_os("NIRI_SESSION_DIR");
        std::env::remove_var("NIRI_SESSION_DIR");
        let cfg = LaunchConfig::default();
        assert_eq!(
            graceful_shutdown_session_path(&cfg),
            default_session_dir().join("last")
        );
        if let Some(s) = saved {
            std::env::set_var("NIRI_SESSION_DIR", s);
        }
    }

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
    fn normalized_single_string_cmdline_splits_for_spawn() {
        let cfg = LaunchConfig::default();
        let w = WindowEntry {
            command: vec!["/opt/google/chrome/chrome --profile-directory=Default --app-id=abc".into()],
            app_id: Some("Google-chrome".into()),
            title: Some("YouTube Music".into()),
            output: "O".into(),
            workspace_idx: 1,
            column: 1,
            tile: 1,
            is_floating: false,
        };
        let argv = resolve_spawn_command(&w, &cfg).expect("resolve");
        assert_eq!(argv[0], "/opt/google/chrome/chrome");
        assert_eq!(argv[1], "--profile-directory=Default");
        assert!(argv.iter().any(|a| a.starts_with("--app-id=")));
    }

    #[test]
    fn title_rule_applies_when_resolve_argv_is_direct_chrome_not_satellite() {
        let cfg = LaunchConfig {
            session: SessionSettings::default(),
            load: LoadSettings::default(),
            launch: vec![LaunchRule {
                app_id: Some("Google-chrome".into()),
                title_contains: Some("YouTube Music".into()),
                resolve: Some("xwayland-satellite".into()),
                command: vec![
                    "/usr/bin/google-chrome-stable".into(),
                    "--profile-directory=Default".into(),
                    "--app-id=pwa".into(),
                ],
            }],
        };
        let w = win(
            Some("Google-chrome"),
            Some("YouTube Music"),
            &["/opt/google/chrome/chrome", "--profile-directory=Default", "--app-id=pwa"],
        );
        let argv = resolve_spawn_command(&w, &cfg).expect("resolve");
        assert_eq!(argv[0], "/usr/bin/google-chrome-stable");
        assert_eq!(argv.len(), 3);
    }

    #[test]
    fn override_by_app_id_only() {
        let cfg = LaunchConfig {
            session: SessionSettings::default(),
            load: LoadSettings::default(),
            launch: vec![LaunchRule {
                app_id: Some("Google-chrome".into()),
                title_contains: None,
                resolve: Some("xwayland-satellite".into()),
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
            session: SessionSettings::default(),
            load: LoadSettings::default(),
            launch: vec![
                LaunchRule {
                    app_id: Some("Google-chrome".into()),
                    title_contains: Some("VK".into()),
                    resolve: Some("xwayland-satellite".into()),
                    command: vec!["chrome-vk".into()],
                },
                LaunchRule {
                    app_id: Some("Google-chrome".into()),
                    title_contains: None,
                    resolve: Some("xwayland-satellite".into()),
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
    fn resolve_listenfd_rule_when_resolve_omitted() {
        let cfg = LaunchConfig {
            session: SessionSettings::default(),
            load: LoadSettings::default(),
            launch: vec![LaunchRule {
                app_id: Some("Google-chrome".into()),
                title_contains: None,
                resolve: None,
                command: vec!["google-chrome-stable".into()],
            }],
        };
        let w = win(Some("Google-chrome"), None, &["wrapper", "-listenfd", "1"]);
        let argv = resolve_spawn_command(&w, &cfg).expect("resolve");
        assert_eq!(argv, vec!["google-chrome-stable"]);
    }

    #[test]
    fn wrong_resolve_skips_to_next_rule() {
        let cfg = LaunchConfig {
            session: SessionSettings::default(),
            load: LoadSettings::default(),
            launch: vec![
                LaunchRule {
                    app_id: Some("Google-chrome".into()),
                    title_contains: None,
                    resolve: Some("other-bin".into()),
                    command: vec!["bad".into()],
                },
                LaunchRule {
                    app_id: Some("Google-chrome".into()),
                    title_contains: None,
                    resolve: Some("xwayland-satellite".into()),
                    command: vec!["good".into()],
                },
            ],
        };
        let w = win(Some("Google-chrome"), None, &["xwayland-satellite", ":1"]);
        let argv = resolve_spawn_command(&w, &cfg).expect("resolve");
        assert_eq!(argv, vec!["good"]);
    }

    #[test]
    fn portable_cmd_ignores_config() {
        let cfg = LaunchConfig {
            session: SessionSettings::default(),
            load: LoadSettings::default(),
            launch: vec![LaunchRule {
                app_id: Some("foot".into()),
                title_contains: None,
                resolve: None,
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

    #[test]
    fn command_string_splits_args() {
        let s = r#"
[[launch]]
app_id = "org.example.app"
command = "flatpak run org.firefox --new-window"
"#;
        let cfg: LaunchConfig = toml::from_str(s).expect("parse");
        assert_eq!(
            cfg.launch[0].command,
            vec!["flatpak", "run", "org.firefox", "--new-window"]
        );
    }

    #[test]
    fn command_string_quoted_token() {
        let s = r#"
[[launch]]
app_id = "x"
command = "\"/opt/My App/foo\" --bar"
"#;
        let cfg: LaunchConfig = toml::from_str(s).expect("parse");
        assert_eq!(cfg.launch[0].command, vec!["/opt/My App/foo", "--bar"]);
    }
}

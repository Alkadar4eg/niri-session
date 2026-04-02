use std::io;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("NIRI_SOCKET is not set; run niri-session-manage inside an active niri session")]
    NiriSocketMissing,

    #[error("IPC I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("niri IPC error: {0}")]
    Niri(String),

    #[error("unexpected IPC response: {0}")]
    UnexpectedResponse(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("specify a mode: --save [PATH], --load [PATH], --graceful-shutdown, or --load-last")]
    NoMode,

    #[error("cannot save and load at the same time")]
    AmbiguousMode,

    #[error("cannot read /proc/{pid}/cmdline: {source}")]
    ProcCmdline { pid: i32, source: io::Error },

    #[error("no workspace for window id {0}")]
    MissingWorkspace(u64),

    #[error("spawn failed: {0}")]
    Spawn(String),

    #[error("empty command")]
    EmptyCommand,

    #[error("config file not found: {}", .0.display())]
    ConfigNotFound(PathBuf),

    #[error("invalid TOML in {}: {msg}", .path.display())]
    ConfigToml { path: PathBuf, msg: String },

    #[error("invalid config {}: {msg}", .path.display())]
    ConfigInvalid { path: PathBuf, msg: String },

    #[error(
        "no [[launch]] rule matches this window; saved argv is not portable (e.g. -listenfd). Add a rule with app_id/title_contains and resolve=… (basename) or resolve=\"-listenfd\" — app_id={app_id:?} title={title:?} cmd={cmd:?}"
    )]
    MissingLaunchOverride {
        cmd: Vec<String>,
        app_id: Option<String>,
        title: Option<String>,
    },

    #[error("восстановление завершено с ошибками: {count} окон (детали выше)")]
    RestorePartial { count: usize },
}

pub type Result<T> = std::result::Result<T, Error>;

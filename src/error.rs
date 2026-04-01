use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("NIRI_SOCKET is not set; run niri-session inside an active niri session")]
    NiriSocketMissing,

    #[error("IPC I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("niri IPC error: {0}")]
    Niri(String),

    #[error("unexpected IPC response: {0}")]
    UnexpectedResponse(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("save or load path must be specified; use --save or --load")]
    NoMode,

    #[error("cannot save and load at the same time")]
    AmbiguousMode,

    #[error("cannot read /proc/{pid}/cmdline: {source}")]
    ProcCmdline { pid: i32, source: io::Error },

    #[error("no workspace for window id {0}")]
    MissingWorkspace(u64),

    #[error("window position missing in layout (not tiled?)")]
    MissingLayoutPosition,

    #[error("spawn failed: {0}")]
    Spawn(String),

    #[error("timed out waiting for window (pid {pid})")]
    WindowTimeout { pid: u32 },

    #[error("failed to align window position after {0} attempts")]
    LayoutAlignFailed(u32),

    #[error("empty command")]
    EmptyCommand,
}

pub type Result<T> = std::result::Result<T, Error>;

mod cmdline_policy;
mod error;
mod ipc;
mod launch_config;
mod proc_cmdline;
mod restore;
mod session;
mod snapshot;

use std::fs;
use std::path::PathBuf;

use clap::Parser;

use crate::error::{Error, Result};
use crate::restore::Timing;

#[derive(Parser, Debug)]
#[command(name = "niri-session", version, about = "Save and restore niri window sessions (JSON)")]
struct Cli {
    /// Write current session to this JSON file
    #[arg(short = 's', long = "save", conflicts_with = "load_path")]
    save_path: Option<PathBuf>,

    /// Restore session from this JSON file
    #[arg(short = 'l', long = "load", conflicts_with = "save_path")]
    load_path: Option<PathBuf>,

    /// TOML with [[launch]] rules (app_id / title_contains → command). Default: ~/.config/niri/niri-session.conf
    #[arg(long = "config", value_name = "PATH")]
    config_path: Option<PathBuf>,

    /// Poll interval while waiting for a new window after spawn (ms)
    #[arg(
        long = "spawn-poll-ms",
        default_value_t = 50,
        env = "NIRI_SESSION_SPAWN_POLL_MS"
    )]
    spawn_poll_ms: u64,

    /// Timeout waiting for a window to appear after spawn (ms)
    #[arg(
        long = "spawn-timeout-ms",
        default_value_t = 120_000,
        env = "NIRI_SESSION_SPAWN_TIMEOUT_MS"
    )]
    spawn_timeout_ms: u64,

    /// Pause after IPC actions that change focus/layout (ms)
    #[arg(
        long = "ipc-settle-ms",
        default_value_t = 80,
        env = "NIRI_SESSION_IPC_SETTLE_MS"
    )]
    ipc_settle_ms: u64,

    /// Delay after spawn before the first window poll (ms)
    #[arg(
        long = "spawn-start-delay-ms",
        default_value_t = 0,
        env = "NIRI_SESSION_SPAWN_START_DELAY_MS"
    )]
    spawn_start_delay_ms: u64,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("niri-session: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match (&cli.save_path, &cli.load_path) {
        (Some(path), None) => cmd_save(path),
        (None, Some(path)) => cmd_load(path, &cli),
        (None, None) => Err(Error::NoMode),
        (Some(_), Some(_)) => Err(Error::AmbiguousMode),
    }
}

fn cmd_save(path: &PathBuf) -> Result<()> {
    let mut socket = ipc::connect()?;
    let session = snapshot::capture(&mut socket)?;
    let json = serde_json::to_string_pretty(&session)?;
    fs::write(path, json)?;
    Ok(())
}

fn cmd_load(path: &PathBuf, cli: &Cli) -> Result<()> {
    let data = fs::read_to_string(path)?;
    let session: session::SessionFile = serde_json::from_str(&data)?;
    let timings = Timing::from_values(
        cli.spawn_poll_ms,
        cli.spawn_timeout_ms,
        cli.ipc_settle_ms,
        cli.spawn_start_delay_ms,
    );
    let launch_cfg = launch_config::load(cli.config_path.as_deref())?;
    let mut socket = ipc::connect()?;
    restore::restore(&mut socket, &session, &timings, &launch_cfg)
}

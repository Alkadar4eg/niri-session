mod cmdline_policy;
mod error;
mod ipc;
mod launch_config;
mod notify_user;
mod proc_cmdline;
mod restore;
mod session;
mod snapshot;

use std::fs;
use std::path::PathBuf;

use clap::Parser;

use crate::error::{Error, Result};
use crate::launch_config::{
    LaunchConfig, DEFAULT_IPC_SETTLE_MS, DEFAULT_SPAWN_POLL_MS, DEFAULT_SPAWN_START_DELAY_MS,
    DEFAULT_SPAWN_TIMEOUT_MS,
};
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

    /// Poll interval while waiting for a new window after spawn (ms). Overrides `[load]` / default.
    #[arg(long = "spawn-poll-ms", env = "NIRI_SESSION_SPAWN_POLL_MS")]
    spawn_poll_ms: Option<u64>,

    /// Timeout waiting for a window to appear after spawn (ms). Default 2000; see `[load]` in config.
    #[arg(long = "spawn-timeout-ms", env = "NIRI_SESSION_SPAWN_TIMEOUT_MS")]
    spawn_timeout_ms: Option<u64>,

    /// Pause after IPC actions that change focus/layout (ms)
    #[arg(long = "ipc-settle-ms", env = "NIRI_SESSION_IPC_SETTLE_MS")]
    ipc_settle_ms: Option<u64>,

    /// Delay after spawn before the first window poll (ms)
    #[arg(
        long = "spawn-start-delay-ms",
        env = "NIRI_SESSION_SPAWN_START_DELAY_MS"
    )]
    spawn_start_delay_ms: Option<u64>,

    /// Disable desktop notification when spawn fails or the window does not appear in time
    #[arg(long = "no-notify-on-spawn-failure", default_value_t = false, action = clap::ArgAction::SetTrue)]
    no_notify_on_spawn_failure: bool,
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
    let launch_cfg = launch_config::load(cli.config_path.as_deref())?;
    let timings = merged_timing(cli, &launch_cfg);
    let notify_on_failure = launch_config::merged_notify_on_failure(cli.no_notify_on_spawn_failure, &launch_cfg);
    let mut socket = ipc::connect()?;
    restore::restore(
        &mut socket,
        &session,
        &timings,
        &launch_cfg,
        notify_on_failure,
    )
}

fn merged_timing(cli: &Cli, cfg: &LaunchConfig) -> Timing {
    let l = &cfg.load;
    Timing::from_values(
        cli.spawn_poll_ms
            .or(l.spawn_poll_ms)
            .unwrap_or(DEFAULT_SPAWN_POLL_MS),
        cli.spawn_timeout_ms
            .or(l.spawn_timeout_ms)
            .unwrap_or(DEFAULT_SPAWN_TIMEOUT_MS),
        cli.ipc_settle_ms
            .or(l.ipc_settle_ms)
            .unwrap_or(DEFAULT_IPC_SETTLE_MS),
        cli.spawn_start_delay_ms
            .or(l.spawn_start_delay_ms)
            .unwrap_or(DEFAULT_SPAWN_START_DELAY_MS),
    )
}

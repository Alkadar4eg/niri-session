mod chrome_pwa;
mod cmdline_policy;
mod debug_log;
mod error;
mod ipc;
mod launch_config;
mod notify_user;
mod proc_cmdline;
mod restore;
mod session;
mod snapshot;

use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;

use crate::debug_log::DebugLog;
use crate::error::{Error, Result};
use crate::launch_config::{
    graceful_shutdown_session_path, merged_default_session_dir, resolve_session_file_path,
    LaunchConfig, DEFAULT_IPC_SETTLE_MS, DEFAULT_SESSION_BASENAME, DEFAULT_SPAWN_DEADLINE_MS,
    DEFAULT_SPAWN_START_DELAY_MS,
};
use crate::restore::Timing;

#[derive(Parser, Debug)]
#[command(
    name = "niri-session-manage",
    version,
    about = "Save and restore niri window sessions (JSON)"
)]
struct Cli {
    /// Verbose debug trace on stderr (IPC, windows, commands, timings)
    #[arg(short = 'd', long = "debug", global = true)]
    debug: bool,

    /// Write session JSON to PATH (bare filename → default session directory; see CONFIG.md)
    #[arg(short = 's', long = "save", value_name = "PATH", num_args = 0..=1, conflicts_with = "load")]
    save: Option<Option<PathBuf>>,

    /// Load session JSON from PATH (bare filename → default session directory; no PATH → session.json there)
    #[arg(short = 'l', long = "load", value_name = "PATH", num_args = 0..=1, conflicts_with = "save")]
    load: Option<Option<PathBuf>>,

    /// Save session to `[session].graceful_shutdown_name`, then close all windows via IPC
    #[arg(
        long = "graceful-shutdown",
        default_value_t = false,
        action = clap::ArgAction::SetTrue,
        conflicts_with_all = ["save", "load", "load_last"]
    )]
    graceful_shutdown: bool,

    /// Load session from `[session].graceful_shutdown_name` (same path as `--graceful-shutdown`)
    #[arg(
        long = "load-last",
        default_value_t = false,
        action = clap::ArgAction::SetTrue,
        conflicts_with_all = ["save", "load", "graceful_shutdown"]
    )]
    load_last: bool,

    /// TOML with [[launch]] rules (app_id / title_contains → command). Default: ~/.config/niri-session/niri-session.conf
    #[arg(long = "config", value_name = "PATH")]
    config_path: Option<PathBuf>,

    /// Pause after IPC actions that change focus/layout, and after each spawn (ms)
    #[arg(long = "ipc-settle-ms", env = "NIRI_SESSION_IPC_SETTLE_MS")]
    ipc_settle_ms: Option<u64>,

    /// Extra delay after each successful spawn before the next window (ms); reduces load spikes
    #[arg(
        long = "spawn-start-delay-ms",
        env = "NIRI_SESSION_SPAWN_START_DELAY_MS"
    )]
    spawn_start_delay_ms: Option<u64>,

    /// Do not wait for window mapping after spawn; next window starts immediately (racy for column stacks)
    #[arg(long = "no-await", default_value_t = false, action = clap::ArgAction::SetTrue)]
    no_await: bool,

    /// Max ms to wait for a new window after spawn (when awaiting is enabled); config: [load].spawn_deadline
    #[arg(
        long = "spawn-deadline",
        env = "NIRI_SESSION_SPAWN_DEADLINE_MS",
        value_name = "MS"
    )]
    spawn_deadline_ms: Option<u64>,

    /// Disable desktop notification on launch failures (resolve/spawn/empty command)
    #[arg(long = "no-notify-on-spawn-failure", default_value_t = false, action = clap::ArgAction::SetTrue)]
    no_notify_on_spawn_failure: bool,

    /// Spawn every app even when a matching window already exists on the workspace (overrides skip)
    #[arg(long = "open-forcefully", default_value_t = false, action = clap::ArgAction::SetTrue)]
    open_forcefully: bool,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("niri-session-manage: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    if cli.graceful_shutdown {
        return cmd_graceful_shutdown(&cli);
    }
    if cli.load_last {
        return cmd_load_last(&cli);
    }

    match (&cli.save, &cli.load) {
        (Some(Some(path)), None) => cmd_save(path, &cli),
        (Some(None), None) => cmd_save_default(&cli),
        (None, Some(Some(path))) => cmd_load(path, &cli),
        (None, Some(None)) => cmd_load_default(&cli),
        (None, None) => Err(Error::NoMode),
        _ => Err(Error::AmbiguousMode),
    }
}

fn cmd_graceful_shutdown(cli: &Cli) -> Result<()> {
    let d = DebugLog::new(cli.debug);
    let launch_cfg = launch_config::load(cli.config_path.as_deref(), d)?;
    let path = graceful_shutdown_session_path(&launch_cfg);
    d.log(format!(
        "--graceful-shutdown -> {}",
        path.display()
    ));
    cmd_write_session(&path, cli, d)?;
    let mut socket = ipc::connect(d)?;
    ipc::close_all_windows(&mut socket, d)?;
    Ok(())
}

fn cmd_load_last(cli: &Cli) -> Result<()> {
    let d = DebugLog::new(cli.debug);
    let launch_cfg = launch_config::load(cli.config_path.as_deref(), d)?;
    let path = graceful_shutdown_session_path(&launch_cfg);
    d.log(format!("--load-last -> {}", path.display()));
    cmd_load_resolved(&path, cli, &launch_cfg, d)
}

fn cmd_save_default(cli: &Cli) -> Result<()> {
    let d = DebugLog::new(cli.debug);
    let launch_cfg = launch_config::load(cli.config_path.as_deref(), d)?;
    let path = merged_default_session_dir(&launch_cfg).join(DEFAULT_SESSION_BASENAME);
    d.log(format!("--save without filename: {}", path.display()));
    cmd_write_session(&path, cli, d)
}

fn cmd_save(user_path: &Path, cli: &Cli) -> Result<()> {
    let d = DebugLog::new(cli.debug);
    let launch_cfg = launch_config::load(cli.config_path.as_deref(), d)?;
    let path = resolve_session_file_path(user_path, &launch_cfg);
    d.log(format!(
        "--save user={} -> {}",
        user_path.display(),
        path.display()
    ));
    cmd_write_session(&path, cli, d)
}

fn cmd_write_session(path: &Path, _cli: &Cli, d: DebugLog) -> Result<()> {
    let mut socket = ipc::connect(d)?;
    let session = snapshot::capture(&mut socket, d)?;
    d.log(format!(
        "captured {} windows, {} workspaces, {} outputs",
        session.windows.len(),
        session.workspaces.len(),
        session.outputs.len()
    ));
    let json = serde_json::to_string_pretty(&session)?;
    fs::write(path, &json)?;
    d.log(format!("wrote {} bytes JSON", json.len()));
    Ok(())
}

fn cmd_load_default(cli: &Cli) -> Result<()> {
    let d = DebugLog::new(cli.debug);
    let launch_cfg = launch_config::load(cli.config_path.as_deref(), d)?;
    let path = merged_default_session_dir(&launch_cfg).join(DEFAULT_SESSION_BASENAME);
    d.log(format!("--load without filename: {}", path.display()));
    cmd_load_resolved(&path, cli, &launch_cfg, d)
}

fn cmd_load(user_path: &Path, cli: &Cli) -> Result<()> {
    let d = DebugLog::new(cli.debug);
    let launch_cfg = launch_config::load(cli.config_path.as_deref(), d)?;
    let path = resolve_session_file_path(user_path, &launch_cfg);
    d.log(format!(
        "--load user={} -> {}",
        user_path.display(),
        path.display()
    ));
    cmd_load_resolved(&path, cli, &launch_cfg, d)
}

fn cmd_load_resolved(path: &Path, cli: &Cli, launch_cfg: &LaunchConfig, d: DebugLog) -> Result<()> {
    let data = fs::read_to_string(path)?;
    let session: session::SessionFile = serde_json::from_str(&data)?;
    d.log(format!(
        "parsed session schema={} niri_version={} windows={}",
        session.schema,
        session.niri_version,
        session.windows.len()
    ));
    let timings = merged_timing(cli, launch_cfg);
    d.log(format!(
        "timings: ipc_settle_ms={} spawn_start_delay_ms={} await_spawn={} spawn_deadline_ms={}",
        timings.ipc_settle_ms,
        timings.spawn_start_delay_ms,
        timings.await_spawn,
        timings.spawn_deadline_ms
    ));
    let notify_on_failure =
        launch_config::merged_notify_on_failure(cli.no_notify_on_spawn_failure, launch_cfg);
    d.log(format!("notify_on_launch_failure={notify_on_failure}"));
    let open_forcefully = merged_open_forcefully(cli, launch_cfg);
    d.log(format!("open_forcefully={open_forcefully}"));
    let mut socket = ipc::connect(d)?;
    restore::restore(
        &mut socket,
        &session,
        &timings,
        launch_cfg,
        notify_on_failure,
        open_forcefully,
        d,
    )
}

fn merged_open_forcefully(cli: &Cli, cfg: &LaunchConfig) -> bool {
    cli.open_forcefully || cfg.load.open_forcefully.unwrap_or(false)
}

fn merged_timing(cli: &Cli, cfg: &LaunchConfig) -> Timing {
    let l = &cfg.load;
    let await_spawn = merged_await_spawn(cli, cfg);
    Timing::from_values(
        cli.ipc_settle_ms
            .or(l.ipc_settle_ms)
            .unwrap_or(DEFAULT_IPC_SETTLE_MS),
        cli.spawn_start_delay_ms
            .or(l.spawn_start_delay_ms)
            .unwrap_or(DEFAULT_SPAWN_START_DELAY_MS),
        await_spawn,
        cli.spawn_deadline_ms
            .or(l.spawn_deadline)
            .unwrap_or(DEFAULT_SPAWN_DEADLINE_MS),
    )
}

fn merged_await_spawn(cli: &Cli, cfg: &LaunchConfig) -> bool {
    if cli.no_await {
        return false;
    }
    !cfg.load.no_await.unwrap_or(false)
}

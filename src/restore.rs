//! Restore session from [`SessionFile`](crate::session::SessionFile).
//!
//! Strategy: **fire and forget** — for each window we focus the target monitor/workspace, spawn the
//! process, then continue without waiting for a window or repositioning. Slow starters no longer
//! block the pipeline; users tune pacing with `ipc_settle_ms` and `spawn_start_delay_ms`.

use std::thread;
use std::time::Duration;

use niri_ipc::socket::Socket;
use niri_ipc::{Action, WorkspaceReferenceArg};

use crate::debug_log::DebugLog;
use crate::error::{Error, Result};
use crate::ipc;
use crate::launch_config::{resolve_spawn_command, LaunchConfig};
use crate::notify_user;
use crate::session::{SessionFile, WindowEntry};

/// User-tunable delays during `--load` (focus pacing and gaps between spawns).
#[derive(Debug, Clone)]
pub struct Timing {
    pub ipc_settle_ms: u64,
    /// Extra pause after a successful spawn before handling the next window.
    pub spawn_start_delay_ms: u64,
}

impl Timing {
    pub fn from_values(ipc_settle_ms: u64, spawn_start_delay_ms: u64) -> Self {
        Self {
            ipc_settle_ms,
            spawn_start_delay_ms,
        }
    }
}

pub fn restore(
    socket: &mut Socket,
    session: &SessionFile,
    timings: &Timing,
    launch_cfg: &LaunchConfig,
    notify_on_spawn_failure: bool,
    debug: DebugLog,
) -> Result<()> {
    let sorted: Vec<_> = session.sorted_windows();
    debug.log(format!("restore: {} windows in sorted order", sorted.len()));
    let mut failed = 0usize;
    for win in sorted {
        if let Err(e) = restore_one(
            socket,
            win,
            timings,
            launch_cfg,
            notify_on_spawn_failure,
            debug,
        ) {
            eprintln!("niri-session: окно пропущено: {e}");
            failed += 1;
        }
    }
    debug.log(format!("restore: done, failed={failed}"));
    if failed == 0 {
        Ok(())
    } else {
        Err(Error::RestorePartial { count: failed })
    }
}

fn sleep_ms(ms: u64, debug: DebugLog, label: &str) {
    if ms > 0 {
        debug.log(format!("sleep {ms} ms ({label})"));
        thread::sleep(Duration::from_millis(ms));
    }
}

fn restore_one(
    socket: &mut Socket,
    win: &WindowEntry,
    timings: &Timing,
    launch_cfg: &LaunchConfig,
    notify_on_spawn_failure: bool,
    debug: DebugLog,
) -> Result<()> {
    debug.log(format!(
        "window app_id={:?} title={:?} output={} ws_idx={} col={} tile={} floating={} saved_cmd={:?}",
        win.app_id,
        win.title,
        win.output,
        win.workspace_idx,
        win.column,
        win.tile,
        win.is_floating,
        win.command
    ));

    let command = match resolve_spawn_command(win, launch_cfg) {
        Ok(c) => {
            debug.log(format!("resolve_spawn_command -> {:?}", c));
            c
        }
        Err(e) => {
            debug.log(format!("resolve_spawn_command -> Err({e})"));
            if notify_on_spawn_failure {
                notify_user::spawn_or_window_failure(
                    "niri-session: не удалось подготовить запуск",
                    &e.to_string(),
                );
            }
            return Err(e);
        }
    };

    if command.is_empty() {
        debug.log("empty argv after resolve");
        if notify_on_spawn_failure {
            notify_user::spawn_or_window_failure(
                "niri-session: пустая команда",
                &format!("app_id={:?} title={:?}", win.app_id, win.title),
            );
        }
        return Err(Error::EmptyCommand);
    }

    let cmd_display = command.join(" ");

    ipc::action(
        socket,
        Action::FocusMonitor {
            output: win.output.clone(),
        },
        debug,
    )?;
    sleep_ms(timings.ipc_settle_ms, debug, "after FocusMonitor");

    ipc::action(
        socket,
        Action::FocusWorkspace {
            reference: WorkspaceReferenceArg::Index(win.workspace_idx),
        },
        debug,
    )?;
    sleep_ms(timings.ipc_settle_ms, debug, "after FocusWorkspace");

    let program = &command[0];
    let args: Vec<String> = command.iter().skip(1).cloned().collect();
    debug.log(format!("spawn: program={program:?} args={args:?}"));
    let mut child = match std::process::Command::new(program)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => {
            debug.log(format!("spawn: ok pid={}", c.id()));
            c
        }
        Err(e) => {
            debug.log(format!("spawn: Err({e})"));
            if notify_on_spawn_failure {
                notify_user::spawn_or_window_failure(
                    "niri-session: не удалось запустить процесс",
                    &format!("{cmd_display}\n{}", e),
                );
            }
            return Err(Error::Spawn(e.to_string()));
        }
    };

    thread::spawn(move || {
        let _ = child.wait();
    });

    sleep_ms(timings.ipc_settle_ms, debug, "after spawn (ipc_settle)");
    sleep_ms(
        timings.spawn_start_delay_ms,
        debug,
        "after spawn (spawn_start_delay)",
    );

    debug.log("window restore step done (fire-and-forget)");
    Ok(())
}

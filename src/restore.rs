//! Restore session from [`SessionFile`](crate::session::SessionFile).

use std::thread;
use std::time::{Duration, Instant};

use niri_ipc::socket::Socket;
use niri_ipc::{Action, WorkspaceReferenceArg};

use crate::error::{Error, Result};
use crate::ipc;
use crate::launch_config::{resolve_spawn_command, LaunchConfig};
use crate::notify_user;
use crate::session::{SessionFile, WindowEntry};

/// User-tunable delays to reduce races during `--load`.
#[derive(Debug, Clone)]
pub struct Timing {
    pub spawn_poll_ms: u64,
    pub spawn_timeout_ms: u64,
    pub ipc_settle_ms: u64,
    pub spawn_start_delay_ms: u64,
}

impl Timing {
    pub fn from_values(
        spawn_poll_ms: u64,
        spawn_timeout_ms: u64,
        ipc_settle_ms: u64,
        spawn_start_delay_ms: u64,
    ) -> Self {
        Self {
            spawn_poll_ms,
            spawn_timeout_ms,
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
) -> Result<()> {
    for win in session.sorted_windows() {
        restore_one(
            socket,
            win,
            timings,
            launch_cfg,
            notify_on_spawn_failure,
        )?;
    }
    Ok(())
}

fn sleep_ms(ms: u64) {
    if ms > 0 {
        thread::sleep(Duration::from_millis(ms));
    }
}

fn restore_one(
    socket: &mut Socket,
    win: &WindowEntry,
    timings: &Timing,
    launch_cfg: &LaunchConfig,
    notify_on_spawn_failure: bool,
) -> Result<()> {
    let command = resolve_spawn_command(win, launch_cfg)?;
    if command.is_empty() {
        return Err(Error::EmptyCommand);
    }

    let cmd_display = command.join(" ");

    ipc::action(
        socket,
        Action::FocusMonitor {
            output: win.output.clone(),
        },
    )?;
    sleep_ms(timings.ipc_settle_ms);

    ipc::action(
        socket,
        Action::FocusWorkspace {
            reference: WorkspaceReferenceArg::Index(win.workspace_idx),
        },
    )?;
    sleep_ms(timings.ipc_settle_ms);

    let program = &command[0];
    let args: Vec<String> = command.iter().skip(1).cloned().collect();
    let mut child = match std::process::Command::new(program)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            if notify_on_spawn_failure {
                notify_user::spawn_or_window_failure(
                    "niri-session: не удалось запустить процесс",
                    &format!("{cmd_display}\n{}", e),
                );
            }
            return Err(Error::Spawn(e.to_string()));
        }
    };

    let pid = child.id();
    thread::spawn(move || {
        let _ = child.wait();
    });

    let window_id = match wait_for_pid(socket, pid, timings) {
        Ok(id) => id,
        Err(e) => {
            if notify_on_spawn_failure {
                let reason = match &e {
                    Error::WindowTimeout { pid } => {
                        format!("окно не появилось за {} ms (pid {pid})", timings.spawn_timeout_ms)
                    }
                    _ => e.to_string(),
                };
                notify_user::spawn_or_window_failure(
                    "niri-session: таймаут ожидания окна",
                    &format!("{cmd_display}\n{reason}"),
                );
            }
            return Err(e);
        }
    };
    sleep_ms(timings.ipc_settle_ms);

    ipc::action(socket, Action::FocusWindow { id: window_id })?;
    sleep_ms(timings.ipc_settle_ms);

    ipc::action(
        socket,
        Action::MoveWindowToMonitor {
            id: Some(window_id),
            output: win.output.clone(),
        },
    )?;
    sleep_ms(timings.ipc_settle_ms);

    ipc::action(
        socket,
        Action::MoveWindowToWorkspace {
            window_id: Some(window_id),
            reference: WorkspaceReferenceArg::Index(win.workspace_idx),
            focus: true,
        },
    )?;
    sleep_ms(timings.ipc_settle_ms);

    if win.is_floating {
        ipc::action(
            socket,
            Action::MoveWindowToFloating {
                id: Some(window_id),
            },
        )?;
        sleep_ms(timings.ipc_settle_ms);
        return Ok(());
    }

    align_tiled(socket, window_id, win.column, win.tile, timings)?;

    Ok(())
}

fn wait_for_pid(socket: &mut Socket, pid: u32, timings: &Timing) -> Result<u64> {
    sleep_ms(timings.spawn_start_delay_ms);
    let deadline = Instant::now() + Duration::from_millis(timings.spawn_timeout_ms);
    while Instant::now() < deadline {
        let list = ipc::windows(socket)?;
        for w in list {
            if w.pid == Some(pid as i32) {
                return Ok(w.id);
            }
        }
        sleep_ms(timings.spawn_poll_ms);
    }
    Err(Error::WindowTimeout { pid })
}

fn layout_of(socket: &mut Socket, window_id: u64) -> Result<(usize, usize)> {
    let list = ipc::windows(socket)?;
    let w = list
        .iter()
        .find(|x| x.id == window_id)
        .ok_or_else(|| Error::UnexpectedResponse("window disappeared".into()))?;
    let pos = w
        .layout
        .pos_in_scrolling_layout
        .ok_or(Error::MissingLayoutPosition)?;
    Ok((pos.0, pos.1))
}

fn align_tiled(
    socket: &mut Socket,
    window_id: u64,
    target_col: usize,
    target_tile: usize,
    timings: &Timing,
) -> Result<()> {
    const MAX_STEPS: u32 = 512;
    for _ in 0..MAX_STEPS {
        let (c, t) = layout_of(socket, window_id)?;
        if c == target_col && t == target_tile {
            return Ok(());
        }

        ipc::action(socket, Action::FocusWindow { id: window_id })?;
        sleep_ms(timings.ipc_settle_ms);

        if c != target_col {
            if c > target_col {
                ipc::action(socket, Action::MoveColumnLeft {})?;
            } else {
                ipc::action(socket, Action::MoveColumnRight {})?;
            }
        } else if t != target_tile {
            if t > target_tile {
                ipc::action(socket, Action::MoveWindowUp {})?;
            } else {
                ipc::action(socket, Action::MoveWindowDown {})?;
            }
        }
        sleep_ms(timings.ipc_settle_ms);
    }
    Err(Error::LayoutAlignFailed(MAX_STEPS))
}

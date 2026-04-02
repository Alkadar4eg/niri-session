//! Restore session from [`SessionFile`](crate::session::SessionFile).
//!
//! По умолчанию после каждого `spawn` ждём появления нового id окна в niri (до `spawn_deadline_ms`),
//! затем переходим к следующему — так сохраняется порядок загрузки. Флаг **`await_spawn = false`**
//! (CLI `--no-await` или `[load].no_await`) даёт режим «запустил и забыл». Для **нескольких тайлов
//! в одной колонке** после следующих окон вызывается `ConsumeWindowIntoColumn` (см. `restore_column_stack`);
//! без ожидания этот шаг может сработать до появления окна.

use std::collections::HashSet;
use std::thread;
use std::time::{Duration, Instant};

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
    /// Wait for a new window id in niri after each successful spawn before continuing.
    pub await_spawn: bool,
    /// Upper bound on polling for that new window (milliseconds).
    pub spawn_deadline_ms: u64,
}

impl Timing {
    pub fn from_values(
        ipc_settle_ms: u64,
        spawn_start_delay_ms: u64,
        await_spawn: bool,
        spawn_deadline_ms: u64,
    ) -> Self {
        Self {
            ipc_settle_ms,
            spawn_start_delay_ms,
            await_spawn,
            spawn_deadline_ms,
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
    let sorted = session.sorted_windows();
    let groups = SessionFile::column_groups(&sorted);
    debug.log(format!(
        "restore: {} windows in {} column/float group(s)",
        sorted.len(),
        groups.len()
    ));
    let mut failed = 0usize;
    for group in groups {
        let r = if group.len() == 1 {
            restore_one(
                socket,
                group[0],
                timings,
                launch_cfg,
                notify_on_spawn_failure,
                debug,
            )
        } else {
            restore_column_stack(
                socket,
                &group,
                timings,
                launch_cfg,
                notify_on_spawn_failure,
                debug,
            )
        };
        if let Err(e) = r {
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

fn window_ids(socket: &mut Socket, debug: DebugLog) -> Result<HashSet<u64>> {
    Ok(ipc::windows(socket, debug)?
        .into_iter()
        .map(|w| w.id)
        .collect())
}

/// Ждём появления нового id окна после spawn (дифф множеств), не дольше `timings.spawn_deadline_ms`.
fn wait_for_new_window(
    socket: &mut Socket,
    before: &HashSet<u64>,
    timings: &Timing,
    debug: DebugLog,
) -> Result<Option<u64>> {
    let poll = timings.ipc_settle_ms.max(40);
    let deadline = Instant::now() + Duration::from_millis(timings.spawn_deadline_ms);
    let mut first = true;
    loop {
        if !first {
            sleep_ms(poll, debug, "poll new window");
        }
        first = false;
        let after: HashSet<u64> = ipc::windows(socket, debug)?
            .into_iter()
            .map(|w| w.id)
            .collect();
        let mut new_ids: Vec<u64> = after.difference(before).copied().collect();
        if !new_ids.is_empty() {
            new_ids.sort_unstable();
            let chosen = *new_ids.last().expect("non-empty");
            debug.log(format!(
                "new window id(s) after spawn: {new_ids:?}, using {chosen}"
            ));
            return Ok(Some(chosen));
        }
        if Instant::now() >= deadline {
            debug.log("wait_for_new_window: spawn_deadline exceeded, no new id");
            return Ok(None);
        }
    }
}

fn focus_monitor_workspace(socket: &mut Socket, win: &WindowEntry, timings: &Timing, debug: DebugLog) -> Result<()> {
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
    Ok(())
}

fn spawn_program(
    command: &[String],
    cmd_display: &str,
    notify_on_spawn_failure: bool,
    debug: DebugLog,
) -> Result<()> {
    if command.is_empty() {
        return Err(Error::EmptyCommand);
    }
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
    Ok(())
}

/// Несколько окон в одной колонке: spawn по порядку тайлов, затем `ConsumeWindowIntoColumn`.
fn restore_column_stack(
    socket: &mut Socket,
    wins: &[&WindowEntry],
    timings: &Timing,
    launch_cfg: &LaunchConfig,
    notify_on_spawn_failure: bool,
    debug: DebugLog,
) -> Result<()> {
    let col = wins[0].column;
    debug.log(format!(
        "restore_column_stack: {} windows, column index={col} (1-based)",
        wins.len()
    ));

    focus_monitor_workspace(socket, wins[0], timings, debug)?;

    for (i, win) in wins.iter().enumerate() {
        debug.log(format!(
            "restore_column_stack: tile {} / {}",
            i + 1,
            wins.len()
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
            if notify_on_spawn_failure {
                notify_user::spawn_or_window_failure(
                    "niri-session: пустая команда",
                    &format!("app_id={:?} title={:?}", win.app_id, win.title),
                );
            }
            return Err(Error::EmptyCommand);
        }

        let cmd_display = command.join(" ");

        let before_ids = window_ids(socket, debug)?;

        ipc::action(
            socket,
            Action::FocusColumn { index: col },
            debug,
        )?;
        sleep_ms(
            timings.ipc_settle_ms,
            debug,
            "before spawn (FocusColumn)",
        );

        spawn_program(
            &command,
            &cmd_display,
            notify_on_spawn_failure,
            debug,
        )?;

        sleep_ms(timings.ipc_settle_ms, debug, "after spawn (ipc_settle)");
        sleep_ms(
            timings.spawn_start_delay_ms,
            debug,
            "after spawn (spawn_start_delay)",
        );

        if timings.await_spawn {
            if i > 0 {
                let _new_id = wait_for_new_window(socket, &before_ids, timings, debug)?;
                ipc::action(
                    socket,
                    Action::FocusColumn { index: col },
                    debug,
                )?;
                sleep_ms(timings.ipc_settle_ms, debug, "after FocusColumn (before consume)");
                ipc::action(socket, Action::ConsumeWindowIntoColumn {}, debug)?;
                sleep_ms(
                    timings.ipc_settle_ms,
                    debug,
                    "after ConsumeWindowIntoColumn",
                );
            } else {
                let _ = wait_for_new_window(socket, &before_ids, timings, debug)?;
            }
        } else if i > 0 {
            ipc::action(
                socket,
                Action::FocusColumn { index: col },
                debug,
            )?;
            sleep_ms(timings.ipc_settle_ms, debug, "after FocusColumn (before consume)");
            ipc::action(socket, Action::ConsumeWindowIntoColumn {}, debug)?;
            sleep_ms(
                timings.ipc_settle_ms,
                debug,
                "after ConsumeWindowIntoColumn",
            );
        }

        debug.log("restore_column_stack: tile step done");
    }

    Ok(())
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

    focus_monitor_workspace(socket, win, timings, debug)?;

    let before_ids = window_ids(socket, debug)?;

    spawn_program(
        &command,
        &cmd_display,
        notify_on_spawn_failure,
        debug,
    )?;

    sleep_ms(timings.ipc_settle_ms, debug, "after spawn (ipc_settle)");
    sleep_ms(
        timings.spawn_start_delay_ms,
        debug,
        "after spawn (spawn_start_delay)",
    );

    if timings.await_spawn {
        let _ = wait_for_new_window(socket, &before_ids, timings, debug)?;
    }

    debug.log("window restore step done");
    Ok(())
}

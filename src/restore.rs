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
use niri_ipc::{Action, SizeChange, Window, Workspace, WorkspaceReferenceArg};

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
    open_forcefully: bool,
    resume_focused: bool,
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
                open_forcefully,
                debug,
            )
        } else {
            restore_column_stack(
                socket,
                &group,
                timings,
                launch_cfg,
                notify_on_spawn_failure,
                open_forcefully,
                debug,
            )
        };
        if let Err(e) = r {
            eprintln!("niri-session-manage: окно пропущено: {e}");
            failed += 1;
        }
    }
    if resume_focused {
        if let Err(e) = restore_saved_focus(socket, session, timings, debug) {
            debug.log(format!("restore_saved_focus: {e}"));
            eprintln!("niri-session-manage: не удалось вернуть фокус: {e}");
        }
    } else {
        debug.log("restore_saved_focus: skipped (resume_focused=false)");
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

fn resolve_workspace_id(workspaces: &[Workspace], output: &str, workspace_idx: u8) -> Option<u64> {
    workspaces
        .iter()
        .find(|w| w.output.as_deref() == Some(output) && w.idx == workspace_idx)
        .map(|w| w.id)
}

fn identity_matches_floating(saved: &WindowEntry, live: &Window) -> bool {
    match (&saved.app_id, &live.app_id) {
        (Some(a), Some(b)) if a == b => match (&saved.title, &live.title) {
            (Some(st), Some(lt)) => st == lt || lt.contains(st.as_str()) || st.contains(lt.as_str()),
            (None, None) => true,
            _ => false,
        },
        (None, None) => saved.title == live.title,
        _ => false,
    }
}

fn identity_matches_tiled(saved: &WindowEntry, live: &Window) -> bool {
    match (&saved.app_id, &live.app_id) {
        (Some(a), Some(b)) if a == b => true,
        (None, None) => saved.title == live.title,
        _ => false,
    }
}

fn find_live_window_for_saved<'a>(
    saved: &WindowEntry,
    workspaces: &'a [Workspace],
    windows: &'a [Window],
) -> Option<&'a Window> {
    let ws_id = resolve_workspace_id(workspaces, &saved.output, saved.workspace_idx)?;
    windows
        .iter()
        .find(|live| live_window_matches_saved_slot(saved, live, ws_id))
}

/// После восстановления окон — сфокусировать то, что было в фокусе при сохранении.
fn restore_saved_focus(
    socket: &mut Socket,
    session: &SessionFile,
    timings: &Timing,
    debug: DebugLog,
) -> Result<()> {
    let Some(saved) = session.windows.iter().find(|w| w.was_focused) else {
        debug.log("restore_saved_focus: no was_focused entry in session");
        return Ok(());
    };
    let workspaces = ipc::workspaces(socket, debug)?;
    let windows = ipc::windows(socket, debug)?;
    let Some(live) = find_live_window_for_saved(saved, &workspaces, &windows) else {
        debug.log(
            "restore_saved_focus: no matching live window (closed, moved, or title/app_id drift)",
        );
        return Ok(());
    };
    debug.log(format!(
        "restore_saved_focus: FocusWindow id={} (saved app_id={:?} title={:?})",
        live.id, saved.app_id, saved.title
    ));
    ipc::action(
        socket,
        Action::FocusWindow { id: live.id },
        debug,
    )?;
    sleep_ms(
        timings.ipc_settle_ms,
        debug,
        "after FocusWindow (restore_saved_focus)",
    );
    Ok(())
}

fn live_window_matches_saved_slot(saved: &WindowEntry, live: &Window, ws_id: u64) -> bool {
    if live.workspace_id != Some(ws_id) {
        return false;
    }
    if saved.is_floating != live.is_floating {
        return false;
    }
    if saved.is_floating {
        return identity_matches_floating(saved, live);
    }
    match live.layout.pos_in_scrolling_layout {
        Some((col, tile)) if col == saved.column && tile == saved.tile => {
            identity_matches_tiled(saved, live)
        }
        _ => false,
    }
}

fn window_already_open(
    saved: &WindowEntry,
    workspaces: &[Workspace],
    windows: &[Window],
    open_forcefully: bool,
) -> bool {
    if open_forcefully {
        return false;
    }
    let Some(ws_id) = resolve_workspace_id(workspaces, &saved.output, saved.workspace_idx) else {
        return false;
    };
    windows
        .iter()
        .any(|live| live_window_matches_saved_slot(saved, live, ws_id))
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

/// После сборки колонки (или одного тайла): ширина колонки и высоты окон из сессии.
fn apply_saved_tiled_geometry(
    socket: &mut Socket,
    wins: &[&WindowEntry],
    timings: &Timing,
    debug: DebugLog,
) -> Result<()> {
    let Some(first) = wins.first().copied() else {
        return Ok(());
    };
    if first.is_floating {
        return Ok(());
    }
    let need_column = first.column_width.is_some();
    let need_any_height = wins.iter().any(|w| w.window_height.is_some());
    if !need_column && !need_any_height {
        return Ok(());
    }

    focus_monitor_workspace(socket, first, timings, debug)?;

    let col = first.column;
    if need_column {
        if let Some(w) = first.column_width {
            ipc::action(
                socket,
                Action::FocusColumn { index: col },
                debug,
            )?;
            sleep_ms(
                timings.ipc_settle_ms,
                debug,
                "before SetColumnWidth (FocusColumn)",
            );
            ipc::action(
                socket,
                Action::SetColumnWidth {
                    change: SizeChange::SetFixed(w),
                },
                debug,
            )?;
            sleep_ms(timings.ipc_settle_ms, debug, "after SetColumnWidth");
        }
    }

    let workspaces = ipc::workspaces(socket, debug)?;
    let windows = ipc::windows(socket, debug)?;

    for win in wins {
        let Some(h) = win.window_height else {
            continue;
        };
        let Some(ws_id) = resolve_workspace_id(&workspaces, &win.output, win.workspace_idx) else {
            debug.log(format!(
                "apply_saved_tiled_geometry: no workspace for output={} idx={}",
                win.output, win.workspace_idx
            ));
            continue;
        };
        let Some(live) = windows
            .iter()
            .find(|live| live_window_matches_saved_slot(win, live, ws_id))
        else {
            debug.log(format!(
                "apply_saved_tiled_geometry: no live window for col={} tile={}",
                win.column, win.tile
            ));
            continue;
        };
        debug.log(format!(
            "apply_saved_tiled_geometry: SetWindowHeight id={} h={h}",
            live.id
        ));
        ipc::action(
            socket,
            Action::SetWindowHeight {
                id: Some(live.id),
                change: SizeChange::SetFixed(h),
            },
            debug,
        )?;
        sleep_ms(
            timings.ipc_settle_ms,
            debug,
            "after SetWindowHeight",
        );
    }

    Ok(())
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
                    "niri-session-manage: не удалось запустить процесс",
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
    open_forcefully: bool,
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
                        "niri-session-manage: не удалось подготовить запуск",
                        &e.to_string(),
                    );
                }
                return Err(e);
            }
        };

        if command.is_empty() {
            if notify_on_spawn_failure {
                notify_user::spawn_or_window_failure(
                    "niri-session-manage: пустая команда",
                    &format!("app_id={:?} title={:?}", win.app_id, win.title),
                );
            }
            return Err(Error::EmptyCommand);
        }

        let cmd_display = command.join(" ");

        let workspaces = ipc::workspaces(socket, debug)?;
        let windows = ipc::windows(socket, debug)?;
        if window_already_open(win, &workspaces, &windows, open_forcefully) {
            debug.log(format!(
                "skip: window already open (app_id={:?} title={:?})",
                win.app_id, win.title
            ));
            eprintln!(
                "niri-session-manage: пропуск: окно уже открыто (app_id={:?}, title={:?})",
                win.app_id, win.title
            );
            continue;
        }

        let before_ids: HashSet<u64> = windows.iter().map(|w| w.id).collect();

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

    apply_saved_tiled_geometry(socket, wins, timings, debug)?;
    Ok(())
}

fn restore_one(
    socket: &mut Socket,
    win: &WindowEntry,
    timings: &Timing,
    launch_cfg: &LaunchConfig,
    notify_on_spawn_failure: bool,
    open_forcefully: bool,
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
                    "niri-session-manage: не удалось подготовить запуск",
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
                "niri-session-manage: пустая команда",
                &format!("app_id={:?} title={:?}", win.app_id, win.title),
            );
        }
        return Err(Error::EmptyCommand);
    }

    let cmd_display = command.join(" ");

    focus_monitor_workspace(socket, win, timings, debug)?;

    let workspaces = ipc::workspaces(socket, debug)?;
    let windows = ipc::windows(socket, debug)?;
    if window_already_open(win, &workspaces, &windows, open_forcefully) {
        debug.log(format!(
            "skip: window already open (app_id={:?} title={:?})",
            win.app_id, win.title
        ));
        eprintln!(
            "niri-session-manage: пропуск: окно уже открыто (app_id={:?}, title={:?})",
            win.app_id, win.title
        );
        return Ok(());
    }

    let before_ids: HashSet<u64> = windows.iter().map(|w| w.id).collect();

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

    if !win.is_floating {
        apply_saved_tiled_geometry(socket, &[win], timings, debug)?;
    }

    debug.log("window restore step done");
    Ok(())
}

#[cfg(test)]
mod matching_tests {
    use super::*;
    use niri_ipc::WindowLayout;

    fn layout(pos: Option<(usize, usize)>) -> WindowLayout {
        WindowLayout {
            pos_in_scrolling_layout: pos,
            tile_size: (100.0, 100.0),
            window_size: (100, 100),
            tile_pos_in_workspace_view: None,
            window_offset_in_tile: (0.0, 0.0),
        }
    }

    fn win(
        id: u64,
        ws_id: u64,
        app_id: Option<&str>,
        title: Option<&str>,
        pos: Option<(usize, usize)>,
        floating: bool,
    ) -> Window {
        Window {
            id,
            title: title.map(str::to_string),
            app_id: app_id.map(str::to_string),
            pid: Some(1),
            workspace_id: Some(ws_id),
            is_focused: false,
            is_floating: floating,
            is_urgent: false,
            layout: layout(pos),
            focus_timestamp: None,
        }
    }

    fn ws(id: u64, idx: u8, output: &str) -> Workspace {
        Workspace {
            id,
            idx,
            name: None,
            output: Some(output.to_string()),
            is_urgent: false,
            is_active: true,
            is_focused: false,
            active_window_id: None,
        }
    }

    fn entry(
        output: &str,
        workspace_idx: u8,
        column: usize,
        tile: usize,
        floating: bool,
        app_id: Option<&str>,
        title: Option<&str>,
    ) -> WindowEntry {
        WindowEntry {
            command: vec!["x".into()],
            app_id: app_id.map(str::to_string),
            title: title.map(str::to_string),
            output: output.into(),
            workspace_idx,
            column,
            tile,
            is_floating: floating,
            was_focused: false,
            column_width: None,
            window_height: None,
        }
    }

    #[test]
    fn already_open_tiled_same_slot_and_app_id() {
        let saved = entry("HDMI-A-1", 1, 2, 1, false, Some("foot"), None);
        let workspaces = vec![ws(10, 1, "HDMI-A-1")];
        let windows = vec![win(1, 10, Some("foot"), Some("a"), Some((2, 1)), false)];
        assert!(window_already_open(
            &saved,
            &workspaces,
            &windows,
            false
        ));
    }

    #[test]
    fn already_open_false_when_wrong_tile() {
        let saved = entry("HDMI-A-1", 1, 2, 1, false, Some("foot"), None);
        let workspaces = vec![ws(10, 1, "HDMI-A-1")];
        let windows = vec![win(1, 10, Some("foot"), None, Some((2, 2)), false)];
        assert!(!window_already_open(
            &saved,
            &workspaces,
            &windows,
            false
        ));
    }

    #[test]
    fn open_forcefully_disables_skip() {
        let saved = entry("HDMI-A-1", 1, 2, 1, false, Some("foot"), None);
        let workspaces = vec![ws(10, 1, "HDMI-A-1")];
        let windows = vec![win(1, 10, Some("foot"), None, Some((2, 1)), false)];
        assert!(!window_already_open(
            &saved,
            &workspaces,
            &windows,
            true
        ));
    }

    #[test]
    fn floating_matches_app_id_and_title_substring() {
        let saved = entry(
            "HDMI-A-1",
            1,
            1,
            1,
            true,
            Some("org.app"),
            Some("Doc"),
        );
        let workspaces = vec![ws(10, 1, "HDMI-A-1")];
        let windows = vec![win(
            1,
            10,
            Some("org.app"),
            Some("Doc — edited"),
            None,
            true,
        )];
        assert!(window_already_open(
            &saved,
            &workspaces,
            &windows,
            false
        ));
    }
}

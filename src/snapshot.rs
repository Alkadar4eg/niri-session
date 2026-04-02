//! Capture current niri state into [`SessionFile`](crate::session::SessionFile).

use crate::chrome_pwa;
use crate::debug_log::DebugLog;
use crate::error::{Error, Result};
use crate::ipc;
use crate::proc_cmdline::read_cmdline;
use crate::session::{SessionFile, WindowEntry, WorkspaceEntry, SCHEMA_VERSION};
use niri_ipc::socket::Socket;

pub fn capture(socket: &mut Socket, debug: DebugLog) -> Result<SessionFile> {
    let niri_version = ipc::version(socket, debug)?;
    let outputs = ipc::outputs(socket, debug)?;
    let workspaces = ipc::workspaces(socket, debug)?;
    let windows = ipc::windows(socket, debug)?;

    let ws_by_id: std::collections::HashMap<u64, _> =
        workspaces.iter().map(|w| (w.id, w)).collect();

    let mut entries: Vec<WindowEntry> = Vec::new();

    for w in windows {
        debug.log(format!(
            "window id={} app_id={:?} title={:?} pid={:?} workspace_id={:?} floating={} pos_in_layout={:?}",
            w.id,
            w.app_id,
            w.title,
            w.pid,
            w.workspace_id,
            w.is_floating,
            w.layout.pos_in_scrolling_layout
        ));

        let Some(ws_id) = w.workspace_id else {
            debug.log("  -> skip: no workspace_id");
            continue;
        };
        let ws = match ws_by_id.get(&ws_id) {
            Some(ws) => ws,
            None => {
                debug.log(format!(
                    "  -> skip: workspace id {ws_id} not in workspace list"
                ));
                return Err(Error::MissingWorkspace(w.id));
            }
        };
        let Some(output) = ws.output.clone() else {
            debug.log("  -> skip: workspace has no output");
            continue;
        };

        let (column, tile) = match w.layout.pos_in_scrolling_layout {
            Some(p) => p,
            None if w.is_floating => (1usize, 1usize),
            None => {
                debug.log("  -> skip: not floating and no pos_in_scrolling_layout");
                continue;
            }
        };

        let Some(pid) = w.pid else {
            debug.log("  -> skip: no pid");
            continue;
        };

        let mut command = read_cmdline(pid)?;
        chrome_pwa::align_chrome_pwa_argv(w.app_id.as_deref(), &mut command);
        debug.log(format!("  -> /proc/{pid}/cmdline: {:?}", command));
        if command.is_empty() {
            debug.log("  -> skip: empty cmdline");
            continue;
        }

        let (column_width, window_height) =
            if !w.is_floating && w.layout.pos_in_scrolling_layout.is_some() {
                let (tw, th) = w.layout.tile_size;
                let cw = tw.round() as i32;
                let wh = th.round() as i32;
                if cw > 0 && wh > 0 {
                    (Some(cw), Some(wh))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

        debug.log(format!(
            "  -> save: output={output} ws_idx={} col={column} tile={tile} column_width={column_width:?} window_height={window_height:?}",
            ws.idx
        ));
        entries.push(WindowEntry {
            command,
            app_id: w.app_id.clone(),
            title: w.title.clone(),
            output,
            workspace_idx: ws.idx,
            column,
            tile,
            is_floating: w.is_floating,
            was_focused: w.is_focused,
            column_width,
            window_height,
        });
    }

    let workspace_entries: Vec<WorkspaceEntry> = workspaces
        .iter()
        .map(|w| WorkspaceEntry {
            id: w.id,
            idx: w.idx,
            name: w.name.clone(),
            output: w.output.clone(),
        })
        .collect();

    Ok(SessionFile {
        schema: SCHEMA_VERSION,
        niri_version,
        outputs,
        workspaces: workspace_entries,
        windows: entries,
    })
}

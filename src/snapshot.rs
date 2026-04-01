//! Capture current niri state into [`SessionFile`](crate::session::SessionFile).

use crate::error::{Error, Result};
use crate::ipc;
use crate::proc_cmdline::read_cmdline;
use crate::session::{SessionFile, WindowEntry, WorkspaceEntry, SCHEMA_VERSION};
use niri_ipc::socket::Socket;

pub fn capture(socket: &mut Socket) -> Result<SessionFile> {
    let niri_version = ipc::version(socket)?;
    let outputs = ipc::outputs(socket)?;
    let workspaces = ipc::workspaces(socket)?;
    let windows = ipc::windows(socket)?;

    let ws_by_id: std::collections::HashMap<u64, _> =
        workspaces.iter().map(|w| (w.id, w)).collect();

    let mut entries: Vec<WindowEntry> = Vec::new();

    for w in windows {
        let Some(ws_id) = w.workspace_id else {
            continue;
        };
        let ws = ws_by_id.get(&ws_id).ok_or(Error::MissingWorkspace(w.id))?;
        let Some(output) = ws.output.clone() else {
            continue;
        };

        let (column, tile) = match w.layout.pos_in_scrolling_layout {
            Some(p) => p,
            None if w.is_floating => (1usize, 1usize),
            None => continue,
        };

        let Some(pid) = w.pid else {
            continue;
        };

        let command = read_cmdline(pid)?;
        if command.is_empty() {
            continue;
        }

        entries.push(WindowEntry {
            command,
            app_id: w.app_id.clone(),
            title: w.title.clone(),
            output,
            workspace_idx: ws.idx,
            column,
            tile,
            is_floating: w.is_floating,
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

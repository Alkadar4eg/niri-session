//! Blocking niri IPC helpers.

use std::collections::HashMap;

use niri_ipc::socket::Socket;
use niri_ipc::{Action, Output, Request, Response, Window, Workspace};

use crate::debug_log::DebugLog;
use crate::error::{Error, Result};

pub fn connect(debug: DebugLog) -> Result<Socket> {
    let sock = std::env::var_os(niri_ipc::socket::SOCKET_PATH_ENV);
    debug.log(format!(
        "IPC: {}={}",
        niri_ipc::socket::SOCKET_PATH_ENV,
        sock.as_ref()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "<unset>".into())
    ));
    if sock.is_none() {
        return Err(Error::NiriSocketMissing);
    }
    let s = Socket::connect().map_err(Error::from)?;
    debug.log("IPC: socket connected");
    Ok(s)
}

pub fn version(socket: &mut Socket, debug: DebugLog) -> Result<String> {
    debug.log("IPC: Request::Version");
    let reply = socket.send(Request::Version)?;
    match reply {
        Ok(Response::Version(v)) => {
            debug.log(format!("IPC: Response::Version {v:?}"));
            Ok(v)
        }
        Ok(other) => Err(Error::UnexpectedResponse(format!("{other:?}"))),
        Err(msg) => Err(Error::Niri(msg)),
    }
}

pub fn outputs(socket: &mut Socket, debug: DebugLog) -> Result<HashMap<String, Output>> {
    debug.log("IPC: Request::Outputs");
    let reply = socket.send(Request::Outputs)?;
    match reply {
        Ok(Response::Outputs(m)) => {
            debug.log(format!("IPC: Response::Outputs ({} outputs)", m.len()));
            Ok(m)
        }
        Ok(other) => Err(Error::UnexpectedResponse(format!("{other:?}"))),
        Err(msg) => Err(Error::Niri(msg)),
    }
}

pub fn workspaces(socket: &mut Socket, debug: DebugLog) -> Result<Vec<Workspace>> {
    debug.log("IPC: Request::Workspaces");
    let reply = socket.send(Request::Workspaces)?;
    match reply {
        Ok(Response::Workspaces(v)) => {
            debug.log(format!(
                "IPC: Response::Workspaces ({} workspaces)",
                v.len()
            ));
            Ok(v)
        }
        Ok(other) => Err(Error::UnexpectedResponse(format!("{other:?}"))),
        Err(msg) => Err(Error::Niri(msg)),
    }
}

pub fn windows(socket: &mut Socket, debug: DebugLog) -> Result<Vec<Window>> {
    debug.log("IPC: Request::Windows");
    let reply = socket.send(Request::Windows)?;
    match reply {
        Ok(Response::Windows(v)) => {
            debug.log(format!("IPC: Response::Windows ({} windows)", v.len()));
            Ok(v)
        }
        Ok(other) => Err(Error::UnexpectedResponse(format!("{other:?}"))),
        Err(msg) => Err(Error::Niri(msg)),
    }
}

pub fn action(socket: &mut Socket, action: Action, debug: DebugLog) -> Result<()> {
    debug.log(format!("IPC: Request::Action {action:?}"));
    let reply = socket.send(Request::Action(action))?;
    match reply {
        Ok(Response::Handled) => {
            debug.log("IPC: Response::Handled");
            Ok(())
        }
        Ok(other) => Err(Error::UnexpectedResponse(format!("{other:?}"))),
        Err(msg) => Err(Error::Niri(msg)),
    }
}

/// Close all toplevel windows (repeat until none remain).
pub fn close_all_windows(socket: &mut Socket, debug: DebugLog) -> Result<()> {
    loop {
        let wins = windows(socket, debug)?;
        if wins.is_empty() {
            debug.log("close_all_windows: done (0 windows)");
            break;
        }
        debug.log(format!(
            "close_all_windows: closing {} window(s)",
            wins.len()
        ));
        for w in wins {
            action(
                socket,
                Action::CloseWindow { id: Some(w.id) },
                debug,
            )?;
        }
    }
    Ok(())
}

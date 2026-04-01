//! Blocking niri IPC helpers.

use std::collections::HashMap;

use niri_ipc::socket::Socket;
use niri_ipc::{Action, Output, Request, Response, Window, Workspace};

use crate::error::{Error, Result};

pub fn connect() -> Result<Socket> {
    if std::env::var_os(niri_ipc::socket::SOCKET_PATH_ENV).is_none() {
        return Err(Error::NiriSocketMissing);
    }
    Socket::connect().map_err(Error::from)
}

pub fn version(socket: &mut Socket) -> Result<String> {
    let reply = socket.send(Request::Version)?;
    match reply {
        Ok(Response::Version(v)) => Ok(v),
        Ok(other) => Err(Error::UnexpectedResponse(format!("{other:?}"))),
        Err(msg) => Err(Error::Niri(msg)),
    }
}

pub fn outputs(socket: &mut Socket) -> Result<HashMap<String, Output>> {
    let reply = socket.send(Request::Outputs)?;
    match reply {
        Ok(Response::Outputs(m)) => Ok(m),
        Ok(other) => Err(Error::UnexpectedResponse(format!("{other:?}"))),
        Err(msg) => Err(Error::Niri(msg)),
    }
}

pub fn workspaces(socket: &mut Socket) -> Result<Vec<Workspace>> {
    let reply = socket.send(Request::Workspaces)?;
    match reply {
        Ok(Response::Workspaces(v)) => Ok(v),
        Ok(other) => Err(Error::UnexpectedResponse(format!("{other:?}"))),
        Err(msg) => Err(Error::Niri(msg)),
    }
}

pub fn windows(socket: &mut Socket) -> Result<Vec<Window>> {
    let reply = socket.send(Request::Windows)?;
    match reply {
        Ok(Response::Windows(v)) => Ok(v),
        Ok(other) => Err(Error::UnexpectedResponse(format!("{other:?}"))),
        Err(msg) => Err(Error::Niri(msg)),
    }
}

pub fn action(socket: &mut Socket, action: Action) -> Result<()> {
    let reply = socket.send(Request::Action(action))?;
    match reply {
        Ok(Response::Handled) => Ok(()),
        Ok(other) => Err(Error::UnexpectedResponse(format!("{other:?}"))),
        Err(msg) => Err(Error::Niri(msg)),
    }
}

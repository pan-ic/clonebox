use nix::{
    fcntl::OFlag,
    sys::socket::{
        AddressFamily, Backlog, SockFlag, SockProtocol, SockType, UnixAddr, accept4, bind, connect,
        listen, socket,
    },
    unistd,
    unistd::pipe2,
};
use std::{
    fs::Permissions,
    os::{
        fd::{AsFd, AsRawFd, OwnedFd},
        unix::fs::PermissionsExt,
    },
};

use crate::state::get_bundle_path;

use crate::error::{CoreError, RuntimeError};

pub(crate) struct Runtime {
    pub(crate) child_end: Option<OwnedFd>,
    pub(crate) parent_end: Option<OwnedFd>,
    pub(crate) unix_sock: Option<OwnedFd>,
}

impl Runtime {
    pub(crate) fn new(
        child_end: Option<OwnedFd>,
        parent_end: Option<OwnedFd>,
        unix_sock: Option<OwnedFd>,
    ) -> Self {
        Runtime {
            child_end,
            parent_end,
            unix_sock,
        }
    }

    pub(crate) fn parent_child_pipe(&mut self) -> Result<(), CoreError> {
        let (child_end, parent_end) =
            pipe2(OFlag::O_CLOEXEC).map_err(RuntimeError::Pipe2Failure)?;
        self.parent_end = Some(parent_end);
        self.child_end = Some(child_end);

        Ok(())
    }

    pub(crate) fn freeze_child(&self) -> Result<(), CoreError> {
        let mut buf: [u8; 1] = [0u8; 1];

        if let Some(child_end) = &self.child_end {
            unistd::read(child_end.as_fd(), &mut buf).map_err(RuntimeError::Read)?;
        }

        Ok(())
    }

    pub(crate) fn unfreeze_child(&self) -> Result<(), CoreError> {
        let buf: [u8; 1] = [0u8; 1];

        if let Some(parent_end) = &self.parent_end {
            unistd::write(parent_end.as_fd(), &buf).map_err(RuntimeError::Write)?;
        }

        Ok(())
    }

    pub(crate) fn parent_proc_socket(&mut self, container_id: &str) -> Result<(), CoreError> {
        let unix_sock = get_socket_fd()?;
        let fd_path = get_socket_path(container_id);

        bind(
            unix_sock.as_raw_fd(),
            &UnixAddr::new(fd_path.as_str()).map_err(RuntimeError::ToUnixAddrFailure)?,
        )
        .map_err(RuntimeError::BindFailure)?;
        std::fs::set_permissions(&fd_path, Permissions::from_mode(0o600))
            .map_err(RuntimeError::Io)?;
        listen(
            &unix_sock,
            Backlog::new(1).map_err(RuntimeError::BacklogFailure)?,
        )
        .map_err(RuntimeError::ListenFailure)?;

        accept4(unix_sock.as_raw_fd(), SockFlag::SOCK_CLOEXEC)
            .map_err(RuntimeError::Accept4Failure)?;

        self.unix_sock = Some(unix_sock);

        Ok(())
    }
}

pub(crate) fn get_socket_fd() -> Result<OwnedFd, CoreError> {
    Ok(socket(
        AddressFamily::Unix,
        SockType::Stream,
        SockFlag::SOCK_CLOEXEC,
        None::<SockProtocol>,
    )
    .map_err(RuntimeError::CreateSocketFailure)?)
}

pub(crate) fn get_socket_path(container_id: &str) -> String {
    format!("{}/start.sk", get_bundle_path(container_id))
}

pub(crate) fn connect_create_process(container_id: &str) -> Result<(), CoreError> {
    let fd = get_socket_fd()?;
    let fd_path = get_socket_path(container_id);

    connect(
        fd.as_raw_fd(),
        &UnixAddr::new(fd_path.as_str()).map_err(RuntimeError::ToUnixAddrFailure)?,
    )
    .map_err(RuntimeError::ConnectFailure)?;

    Ok(())
}

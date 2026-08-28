use anyhow::Context;
use nix::{
    fcntl::OFlag,
    sys::socket::{
        AddressFamily, Backlog, SockFlag, SockProtocol, SockType, UnixAddr, accept4, bind, connect,
        listen, socket,
    },
    unistd::{pipe2, read, write},
};
use std::{
    os::{ 
        fd::{AsFd, AsRawFd, OwnedFd},
        unix::fs::PermissionsExt,
    },
    fs::Permissions,
};

use crate::state::get_bundle_path;

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

    pub(crate) fn parent_child_pipe(&mut self) -> anyhow::Result<()> {
        let (child_end, parent_end) =
            pipe2(OFlag::O_CLOEXEC).context("parent_child_pipe: failed to pipe2()")?;
        self.parent_end = Some(parent_end);
        self.child_end = Some(child_end);

        Ok(())
    }

    pub(crate) fn freeze_child(&self) -> anyhow::Result<()> {
        let mut buf: [u8; 1] = [0u8; 1];

        if let Some(child_end) = &self.child_end {
            read(child_end.as_fd(), &mut buf).context("freeze_child: failed to read()")?;
        }

        Ok(())
    }

    pub(crate) fn unfreeze_child(&self) -> anyhow::Result<()> {
        let buf: [u8; 1] = [0u8; 1];

        if let Some(parent_end) = &self.parent_end {
            write(parent_end.as_fd(), &buf).context("unfreeze_child: failed to write()")?;
        }

        Ok(())
    }

    pub(crate) fn parent_proc_socket(&mut self, container_id: &str) -> anyhow::Result<()> {
        let unix_sock = get_socket_fd().context("parent_proc_socket: failed to build socket")?;
        let fd_path = get_socket_path(container_id);

        bind(
            unix_sock.as_raw_fd(),
            &UnixAddr::new(fd_path.as_str())
                .context("parent_proc_socket: failed to create unix address")?,
        )
        .context("parent_proc_socket: failed to bind")?;
        std::fs::set_permissions(&fd_path, Permissions::from_mode(0o600))
            .context("parent_proc_socket: failed to chmod socket")?;
        listen(
            &unix_sock,
            Backlog::new(1).context("parent_proc_socket: failed to create backlog type")?,
        )
        .context("parent_proc_socket: failed to listen to")?;

        accept4(unix_sock.as_raw_fd(), SockFlag::SOCK_CLOEXEC)
            .context("parent_proc_socket: failed to accept")?;

        self.unix_sock = Some(unix_sock);

        Ok(())
    }
}

pub(crate) fn get_socket_fd() -> anyhow::Result<OwnedFd> {
    socket(
        AddressFamily::Unix,
        SockType::Stream,
        SockFlag::SOCK_CLOEXEC,
        None::<SockProtocol>,
    )
    .context("get_socket_fd: failed to create socket")
}

pub(crate) fn get_socket_path(container_id: &str) -> String {
    format!("{}/start.sk", get_bundle_path(container_id))
}

pub(crate) fn connect_create_process(container_id: &str) -> anyhow::Result<()> {
    let fd = get_socket_fd().context("connect_create_process: failed to build socket")?;
    let fd_path = get_socket_path(container_id);

    connect(
        fd.as_raw_fd(),
        &UnixAddr::new(fd_path.as_str())
            .context("connect_create_process: failed to create unix address")?,
    )
    .context("connect_create_process: failed to connect to socket")?;

    Ok(())
}

use anyhow;
use thiserror;

#[derive(thiserror::Error, Debug)]
pub enum CoreError {
    #[error("{0} already exists")]
    ContainerAlreadyExists(String),
    #[error("{0} already stopped")]
    ContainerAlreadyStopped(String),
    #[error("{0} doesn't exists")]
    ContainerNotFound(String),
    #[error("{0} not created or already running")]
    ContainerNotCreated(String),
    #[error("{0} not running")]
    ContainerNotRunning(String),
    #[error("{0} not paused")]
    ContainerNotPaused(String),
    #[error("{0} not stopped")]
    ContainerNotStopped(String),
    #[error("failed to clean {0}")]
    CleanupFailure(#[source] std::io::Error, String),
    #[error(transparent)]
    ConfigError(#[from] ConfigError),
    #[error(transparent)]
    StateError(#[from] StateError),
    #[error(transparent)]
    SystemError(#[from] SystemError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("config.json: root.path is required")]
    MissingRootPath,
    #[error("config.json: process.args is required")]
    MissingProcessArgs,
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[error("json error")]
    Json(#[from] serde_json::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum StateError {
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[error("json error")]
    Json(#[from] serde_json::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum SystemError {
    #[error(transparent)]
    RuntimeError(#[from] RuntimeError),
    #[error(transparent)]
    CloneError(#[from] CloneError),
    #[error(transparent)]
    NetworkError(#[from] NetworkError),
    #[error(transparent)]
    CgroupError(#[from] CgroupError),
    #[error(transparent)]
    NamespaceError(#[from] NamespaceError),
    #[error(transparent)]
    LogError(#[from] LogError),
    #[error(transparent)]
    EventError(#[from] EventError),
    #[error("waitpid failed")]
    Wait(#[source] nix::errno::Errno),
    #[error("io error: {1}")]
    Io(#[source] std::io::Error, String),
}

#[derive(thiserror::Error, Debug)]
pub enum RuntimeError {
    #[error("failed to accept")]
    Accept4Failure(#[source] nix::errno::Errno),
    #[error("failed to create backlog")]
    BacklogFailure(#[source] nix::errno::Errno),
    #[error("failed to bind")]
    BindFailure(#[source] nix::errno::Errno),
    #[error("failed to create socket")]
    CreateSocketFailure(#[source] nix::errno::Errno),
    #[error("failed to connect")]
    ConnectFailure(#[source] nix::errno::Errno),
    #[error("failed to listen")]
    ListenFailure(#[source] nix::errno::Errno),
    #[error("failed to pipe")]
    Pipe2Failure(#[source] nix::errno::Errno),
    #[error("failed to create UnixAddr")]
    ToUnixAddrFailure(#[source] nix::errno::Errno),
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[error("pipe read failed")]
    Read(#[source] nix::errno::Errno),
    #[error("pipe write failed")]
    Write(#[source] nix::errno::Errno),
}

impl From<RuntimeError> for CoreError {
    fn from(e: RuntimeError) -> Self {
        CoreError::SystemError(SystemError::RuntimeError(e))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum CloneError {
    #[error("CLONE_PIDFD set but no pid_fd provided")]
    NoPidFd,
    #[error("CLONE_CHILD_SETTID set but no child_tid provided")]
    NoChildTid,
    #[error("CLONE_PARENT_SETTID set but no parent_tid provided")]
    NoParentTid,
    #[error("CLONE_SETTLS set but no tls provided")]
    NoTls,
    #[error("CLONE_INTO_CGROUP set but no cgroup fd provided")]
    NoCgroup,
    #[error("clone3 failed")]
    Clone3Failure(#[source] std::io::Error),
}

impl From<CloneError> for CoreError {
    fn from(e: CloneError) -> Self {
        CoreError::SystemError(SystemError::CloneError(e))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum NetworkError {
    #[error("failed to create socket")]
    CreateSocketFailure(#[source] nix::errno::Errno),
    #[error("failed to bind")]
    BindFailure(#[source] nix::errno::Errno),
    #[error("read out of range")]
    ReadOutOfRange,
    #[error("failed to read {0}")]
    ReadFailure(String),
    #[error("fail to send bytes")]
    SendFailure(#[source] nix::errno::Errno),
    #[error("failed to receive bytes")]
    RecvFailure(#[source] nix::errno::Errno),
    #[error("netlink: {0}")]
    NetlinkFailure(String),
    #[error("failed to open")]
    OpenFailure(#[source] std::io::Error),
    #[error("failed to write")]
    WriteFailure(#[source] std::io::Error),
    #[error("command failed wth {0}")]
    CommandFailure(#[source] std::io::Error),
}

impl From<NetworkError> for CoreError {
    fn from(e: NetworkError) -> Self {
        CoreError::SystemError(SystemError::NetworkError(e))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum CgroupError {
    #[error("failed to create {1} cgroup")]
    CreateCgroupDirFailure(#[source] std::io::Error, String),
    #[error("failed to open {1} cgroup")]
    OpenFailure(#[source] std::io::Error, String),
    #[error("failed to read {1}: {2}")]
    ReadToStringFailure(#[source] std::io::Error, String, String),
    #[error("{0}: {1} not available on this system")]
    ResourceNotAvailable(String, String),
    #[error("failed to write into {1} cgroup")]
    WriteFailure(#[source] std::io::Error, String),
}

impl From<CgroupError> for CoreError {
    fn from(e: CgroupError) -> Self {
        CoreError::SystemError(SystemError::CgroupError(e))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum NamespaceError {
    #[error("failed to enter {0} namespace")]
    FailedToEnterNamespace(String),
}

impl From<NamespaceError> for CoreError {
    fn from(e: NamespaceError) -> Self {
        CoreError::SystemError(SystemError::NamespaceError(e))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum LogError {
    #[error("io error")]
    Io(#[from] std::io::Error),
}

impl From<LogError> for CoreError {
    fn from(e: LogError) -> Self {
        CoreError::SystemError(SystemError::LogError(e))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum EventError {
    #[error("failed to send status message")]
    WriteFailure(#[source] std::io::Error),
    #[error("failed to connect to socket {1}")]
    ConnectFailure(#[source] std::io::Error, String),
    #[error("json error")]
    Json(#[from] serde_json::Error),
}

impl From<EventError> for CoreError {
    fn from(e: EventError) -> Self {
        CoreError::SystemError(SystemError::EventError(e))
    }
}

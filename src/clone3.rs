use libc::syscall;
use nix::{sys::signal::Signal, unistd::Pid};
use std::os::fd::RawFd;

pub const CLONE_PIDFD: u64 = 0x00001000;
pub const CLONE_PARENT_SETTID: u64 = 0x00100000;
pub const CLONE_CHILD_SETTID: u64 = 0x01000000;
pub const CLONE_SETTLS: u64 = 0x00080000;
pub const CLONE_INTO_CGROUP: u64 = 0x200000000;
#[allow(dead_code)]
const SYS_CLONE3: i64 = 435;

#[allow(dead_code)]
#[repr(C)]
struct CloneArgs {
    flags: u64,
    pidfd: u64,
    child_tid: u64,
    parent_tid: u64,
    exit_signal: u64,
    stack: u64,
    stack_size: u64,
    tls: u64,
    set_tid: u64,
    set_tid_size: u64,
    cgroup: u64,
}

#[allow(dead_code)]
pub(crate) struct Clone3 {
    flags: u64,
    pid_fd: Option<RawFd>,
    child_tid: Option<Pid>,
    parent_tid: Option<Pid>,
    exit_signal: Option<Signal>,
    stack: Option<Vec<u8>>,
    tls: Option<u64>,
    set_tid: Option<Vec<Pid>>,
    cgroup: Option<RawFd>,
}

#[allow(dead_code)]
impl Clone3 {
    pub fn new(exit_signal: Option<Signal>) -> Self {
        Clone3 {
            flags: 0,
            pid_fd: None,
            child_tid: None,
            parent_tid: None,
            exit_signal,
            stack: None,
            tls: None,
            set_tid: None,
            cgroup: None,
        }
    }

    pub fn flags(mut self, flags: u64) -> Self {
        self.flags |= flags;
        self
    }

    pub(crate) fn stack(mut self, size: usize) -> Self {
        self.stack = Some(vec![0u8; size]);
        self
    }

    pub(crate) fn pidfd(mut self, fd: RawFd) -> Self {
        self.flags |= CLONE_PIDFD;
        self.pid_fd = Some(fd);
        self
    }

    pub(crate) fn child_tid(mut self, id: Pid) -> Self {
        self.flags |= CLONE_CHILD_SETTID;
        self.child_tid = Some(id);
        self
    }

    pub(crate) fn parent_tid(mut self, id: Pid) -> Self {
        self.flags |= CLONE_PARENT_SETTID;
        self.parent_tid = Some(id);
        self
    }

    pub(crate) fn tls(mut self, tls: u64) -> Self {
        self.flags |= CLONE_SETTLS;
        self.tls = Some(tls);
        self
    }

    pub(crate) fn set_tid(mut self, id: Option<Vec<Pid>>) -> Self {
        self.set_tid = id;
        self
    }

    pub(crate) fn cgroup(mut self, fd: RawFd) -> Self {
        self.flags |= CLONE_INTO_CGROUP;
        self.cgroup = Some(fd);
        self
    }

    pub(crate) fn build(self) -> anyhow::Result<Pid> {
        if self.flags & CLONE_PIDFD != 0 && self.pid_fd.is_none() {
            anyhow::bail!("CLONE_PIDFD set but no pid_fd provided")
        }

        if self.flags & CLONE_CHILD_SETTID != 0 && self.child_tid.is_none() {
            anyhow::bail!("CLONE_CHILD_SETTID set but no child_tid provided")
        }

        if self.flags & CLONE_PARENT_SETTID != 0 && self.parent_tid.is_none() {
            anyhow::bail!("CLONE_PARENT_SETTID set but no parent_tid provided")
        }

        if self.flags & CLONE_SETTLS != 0 && self.tls.is_none() {
            anyhow::bail!("CLONE_SETTLS set but no tls provided")
        }

        if self.flags & CLONE_INTO_CGROUP != 0 && self.cgroup.is_none() {
            anyhow::bail!("CLONE_INTO_CGROUP set but no cgroup fd provided")
        }

        let flags = self.flags;
        let pidfd = self.pid_fd.map_or(0, |fd| fd as u64);
        let child_tid = self.child_tid.map_or(0, |pid| pid.as_raw() as u64);
        let parent_tid = self.parent_tid.map_or(0, |pid| pid.as_raw() as u64);
        let exit_signal = self.exit_signal.map_or(0, |signal| signal as u64);
        let (stack_ptr, stack_size) = match &self.stack {
            None => (0u64, 0u64),
            Some(s) => (s.as_ptr() as u64, s.len() as u64),
        };
        let tls = self.tls.map_or(0, |v| v);
        //let tls = self.tls.map_or(0, |v| {v as u64});
        let (set_tid_ptr, set_tid_size) = match self.set_tid {
            None => (0u64, 0u64),
            Some(v) => (v.as_ptr() as u64, v.len() as u64),
        };
        let cgroup = self.cgroup.map_or(0, |fd| fd as u64);

        let clone_args = CloneArgs {
            flags,
            pidfd,
            child_tid,
            parent_tid,
            exit_signal,
            stack: stack_ptr,
            stack_size,
            tls,
            set_tid: set_tid_ptr,
            set_tid_size,
            cgroup,
        };

        let pid = unsafe {
            syscall(
                SYS_CLONE3,
                &clone_args as *const CloneArgs,
                size_of::<CloneArgs>(),
            )
        };

        if pid < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(anyhow::anyhow!(
                "clone3 failed: errno {}: {}",
                errno,
                std::io::Error::last_os_error()
            ));
        }

        Ok(Pid::from_raw(pid as i32))
    }
}

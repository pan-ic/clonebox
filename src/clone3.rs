use anyhow;
use libc::{syscall};
use nix::{
    sys::{wait::waitpid, signal::Signal},
    sched::{CloneCb, CloneFlags, clone},
    unistd::{Pid, execve, getpid, sethostname, write},
};
use std::{
    ffi::{CString, c_int},
    os::fd::RawFd,
};

pub const CLONE_PIDFD: u64          = 0x00001000;
pub const CLONE_PARENT_SETTID: u64  = 0x00100000;
pub const CLONE_CHILD_SETTID: u64   = 0x01000000;
pub const CLONE_SETTLS: u64         = 0x00080000;
pub const CLONE_INTO_CGROUP: u64    = 0x200000000;
#[allow(dead_code)]
const SYS_CLONE3: i64 = 435;

/*
    NOTE: on understanding about clone(), clone3() similarities and differences

    clone(): wrapped by glibc, libc ffi, and nix implementation. Creates a child process and
    start the new process using the callback that is passed to the function. The child inherits
    from the parent only what we allows it to inherits; that using the CloneFlags features. Never
    had the chance to use pid_t *_Nullable parent_tid, void *_Nullable tls,
    pid_t *_Nullable child_tid so idk what they are used for. On the return:
    -the return of the clone call in the parent part is the child PID
    -child has no proper return, it exits with an exit code

    clone3(): not wrapped yet by glibc. Doesn't exist yet in libc (only the sys op code is defined)
    nor nix. Man describe it as a superset of clone(). So it creates a child that inherits everything
    from it parents process (fork style) and start execution at the same point where parent clone call
    was. CloneFlags features are here to control over what it's inherits. But still it copies the parent
    stack (so environment, fd, time, and every might be secrets), solution to reset the satck is to call
    a first execve on itself. On the return:
    -child returns 0 if success
    -parents returns child PID

    diff on call:
    -clone uses: func call back (function pointer in C), stack pointer + stack size, cloneflgs,
                    args that are passed to execve in the child. No clue about the use of other things in C
    -clone3 uses: cleaner: a struct and I guess the size of that struct; this is passed to syscall then. But
    fork style  differentiating the child clone return (0) from the parent (child_pid) first, then impl nix
    style with call_back (even if clone3() does not take a func pointer)

    more diff in man:
        clone()         clone3()        Notes
                           cl_args field
           flags & ~0xff   flags           For most flags; details below
           parent_tid      pidfd           See CLONE_PIDFD
           child_tid       child_tid       See CLONE_CHILD_SETTID
           parent_tid      parent_tid      See CLONE_PARENT_SETTID
           flags & 0xff    exit_signal
           stack           stack
           ---             stack_size
           tls             tls             See CLONE_SETTLS
           ---             set_tid         See below for details
           ---             set_tid_size
           ---             cgroup          See CLONE_INTO_CGROUP

    syscall(): here I'm not really sure, first argument is the SYS op code, like the call to the assembly func, the
    second is a variadic argument so theorically you passes everything that is needed. Now the order is like a surprise
    I guess. So logic would be to follow the pointer on the struct raw bytes as first variadic arg and the the size of
    that struct. On the return value, it is supposed to be the return defined by the clone 3 assembly func so,
    PID in parent and 0 in child
        
    MEMO:

    int clone(int (*fn)(void *_Nullable), void *stack, int flags,
                 void *_Nullable arg, ...  /* pid_t *_Nullable parent_tid,
                                              void *_Nullable tls,
                                              pid_t *_Nullable child_tid */ );

    pub unsafe fn clone(cb: CloneCb<'_>,
        stack: &mut [u8],
        flags: CloneFlags,
        signal: Option<c_int>,
        ) -> Result<Pid>

    long syscall(SYS_clone3, struct clone_args *cl_args, size_t size);

    pub const SYS_clone3: c_long = 435;

    pub unsafe extern "C" fn syscall(num: c_long, ...) -> c_long

    struct clone_args {
               u64 flags;        /* Flags bit mask */
               u64 pidfd;        /* Where to store PID file descriptor
                                    (int *) */
               u64 child_tid;    /* Where to store child TID,
                                    in child's memory (pid_t *) */
               u64 parent_tid;   /* Where to store child TID,
                                    in parent's memory (pid_t *) */
               u64 exit_signal;  /* Signal to deliver to parent on
                                    child termination */
               u64 stack;        /* Pointer to lowest byte of stack */
               u64 stack_size;   /* Size of stack */
               u64 tls;          /* Location of new TLS */
               u64 set_tid;      /* Pointer to a pid_t array
                                    (since Linux 5.5) */
               u64 set_tid_size; /* Number of elements in set_tid
                                    (since Linux 5.5) */
               u64 cgroup;       /* File descriptor for target cgroup
                                    of child (since Linux 5.7) */
    };
*/

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

    pub(crate) fn build<'a>(self, mut cb: Box<dyn FnMut() -> isize + 'a>) ->  anyhow::Result<Pid> {
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
        let pidfd = self.pid_fd.map_or(0, |fd| {fd as u64});
        let child_tid = self.child_tid.map_or(0, |pid| {pid.as_raw() as u64});
        let parent_tid = self.parent_tid.map_or(0, |pid| {pid.as_raw() as u64});
        let exit_signal = self.exit_signal.map_or(0, |signal| {signal as u64});
        let (stack_ptr, stack_size) = match &self.stack {
            None => (0u64, 0u64),
            Some(s) => (s.as_ptr() as u64, s.len() as u64),
        };
        let tls = self.tls.map_or(0, |v| {v as u64});
        let (set_tid_ptr, set_tid_size) = match self.set_tid {
            None => (0u64, 0u64),
            Some(v) => (v.as_ptr() as u64, v.len() as u64),
        };
        let cgroup = self.cgroup.map_or(0, |fd| {fd as u64}) as u64;

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
            syscall(SYS_CLONE3, &clone_args as *const CloneArgs, size_of::<CloneArgs>())
        };

        if pid < 0 {
            let errno = unsafe { *libc::__errno_location() };
            println!("errno: {}", errno);
            println!("error: {}", std::io::Error::from_raw_os_error(errno));
            return Err(anyhow::anyhow!("clone3 failed: {}", std::io::Error::last_os_error()))
        }

        if pid == 0 {
            let res = cb();
            unsafe { libc::_exit(res as i32) };
        }
        
        Ok(Pid::from_raw(pid as i32))
    }
}

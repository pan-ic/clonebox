#[cfg(target_os = "linux")]
use nix::{
    sched::{CloneFlags, setns},
    sys::{
        signal::Signal,
        wait::{WaitStatus, waitpid},
    },
    unistd::{ForkResult, Pid, execve, fork, getpid, write as nix_write},
};
use std::{
    ffi::CString,
    fs::{File, Permissions, create_dir_all, remove_dir, remove_dir_all},
    os::{
        fd::{AsFd, AsRawFd},
        unix::fs::PermissionsExt,
    },
    path::Path,
};

use crate::clone3::Clone3;

use crate::cgroup::{
    create_cgroup, get_app_cgroup_path, get_child_cgroup_path, get_root_cgroup_path,
    init_resources, set_cgroup,
};

use crate::namespace::{
    do_default_mounts, do_pivot_root, make_child_private, post_pivot_mount, pre_pivot_mount,
    set_child_hostname,
};

use crate::network::create_network;

use crate::state::{
    ContainerState, State, get_bundle_path, read_state_file, update_state, write_state_file,
};

use crate::runtime::{Runtime, connect_create_process};

use crate::event::Event;

use crate::config::Config;

use crate::logger::{open_log_file, write_log_file};

use crate::error::{CoreError, NamespaceError, SystemError};

fn cleanup(container_id: &str, force: bool) -> Result<(), CoreError> {
    let cgroup_path = format!("/sys/fs/cgroup/clonebox/{}", container_id);
    let bundle_path = get_bundle_path(container_id);

    if force {
        remove_dir(&cgroup_path).ok();
        remove_dir_all(bundle_path).ok();
        return Ok(());
    }

    remove_dir(&cgroup_path).map_err(|e| CoreError::CleanupFailure(e, "cgroup".to_string()))?;
    remove_dir_all(&bundle_path).map_err(|e| CoreError::CleanupFailure(e, "bundle".to_string()))?;

    Ok(())
}

fn to_cstring_vec(v: Vec<String>) -> Vec<CString> {
    v.iter()
        .map(|s| CString::new(s.as_str()).unwrap())
        .collect()
}

#[allow(unused)]
fn container_exec(
    args: Option<Vec<String>>,
    env: Option<Vec<String>>,
    cwd: Option<&str>,
    mut log_fd: &mut File,
) -> ! {
    let args = args.unwrap_or_default();

    if args.is_empty() {
        let _ = nix_write(std::io::stderr(), b"no args provided\n");
        unsafe { libc::_exit(1) }
    }

    let path = &args[0].clone();

    let c_path = CString::new(path.as_str()).unwrap_or_else(|_| {
        let _ = nix_write(std::io::stderr(), b"invalid path\n");
        unsafe { libc::_exit(1) }
    });
    let ca: Vec<CString> = to_cstring_vec(args);
    let ce: Vec<CString> = to_cstring_vec(env.unwrap_or_default());

    if let Some(cwd) = cwd {
        let c_cwd = CString::new(cwd).unwrap_or_else(|_| {
            let _ = nix_write(std::io::stderr(), b"invalid cwd\n");
            unsafe { libc::_exit(1) }
        });
        unsafe { libc::chdir(c_cwd.as_ptr()) };
    }

    let Err(e) = execve(&c_path, &ca, &ce);
    let e = format!("Execve failure: {}\n", e);
    #[allow(unused)]
    write_log_file(log_fd, &e);
    unsafe {
        libc::_exit(1);
    }
}

#[cfg(target_os = "linux")]
pub fn create(
    container_id: &str,
    config_path: &str,
    socket_path: Option<&str>,
) -> Result<(), CoreError> {
    let bundle_path = get_bundle_path(container_id);
    if Path::new(&bundle_path).exists() {
        return Err(CoreError::ContainerAlreadyExists(container_id.to_string()));
    }

    let config = Config::load(config_path)?;

    create_dir_all(&bundle_path).map_err(|e| SystemError::Io(e, bundle_path.to_string()))?;
    std::fs::set_permissions(&bundle_path, Permissions::from_mode(0o700))
        .map_err(|e| SystemError::Io(e, bundle_path.to_string()))?;

    let mut log_fd = open_log_file(&bundle_path)?;
    let mut runtime = Runtime::new(None, None, None);
    let state = State::new(
        config.get_oci_version().to_string(),
        container_id.to_string(),
        ContainerState::Creating,
        None,
        bundle_path.clone(),
        None,
    );
    #[allow(unused)]
    let mut event = Event::new(container_id.to_string(), ContainerState::Creating, Some(0));

    if let Some(sp) = socket_path {
        event.send_event(sp)?;
    }
    write_state_file(container_id, &state)?;

    //TODO: replace what can be repaced by config parsing
    let new_root = config.get_root_path();
    let empty = vec![];
    let mounts = config.get_mounts().unwrap_or(&empty);

    let (_, child_cgroup_fd) = create_cgroup(container_id)?;
    let resources = vec!["cpu", "memory"];
    init_resources(get_root_cgroup_path(), &resources)?;
    init_resources(get_app_cgroup_path(), &resources)?;

    let clone_flags: CloneFlags = CloneFlags::CLONE_NEWPID
        | CloneFlags::CLONE_NEWUTS
        | CloneFlags::CLONE_NEWNS
        | CloneFlags::CLONE_NEWNET;
    let signal: Option<Signal> = Some(nix::sys::signal::Signal::SIGCHLD);

    runtime.parent_child_pipe()?;

    let child_pid: Pid = Clone3::new(signal)
        .flags(clone_flags.bits() as u64)
        .cgroup(child_cgroup_fd.as_raw_fd())
        .build()?;

    if child_pid == Pid::from_raw(0) {
        make_child_private();
        set_child_hostname(&config.get_hostname().unwrap_or("none".to_string()));
        let new_root = pre_pivot_mount(&bundle_path, new_root, mounts);
        do_pivot_root(&new_root);
        post_pivot_mount(mounts);
        do_default_mounts();

        runtime.freeze_child()?;

        container_exec(
            config.get_process_args(),
            config.get_process_env(),
            Some(config.get_process_cwd()),
            &mut log_fd,
        );
    }

    create_network(container_id, &child_pid)?;

    let child_cgroup_path = get_child_cgroup_path(container_id);
    set_cgroup(&child_cgroup_path, "cpu", "max", "100000 100000")?;
    set_cgroup(&child_cgroup_path, "memory", "max", "256M")?;

    if let Some(sp) = socket_path {
        event.update_state(ContainerState::Created);
        event.send_event(sp)?;
    }
    update_state(container_id, |s| s.set_created(child_pid.as_raw()))?;

    runtime.parent_proc_socket(container_id)?;
    runtime.unfreeze_child()?;

    let child_return = waitpid(child_pid, None).map_err(SystemError::Wait)?;
    //TODO: proper child retrun handling
    //pub enum WaitStatus {
    //  Exited(Pid, i32),
    //  Signaled(Pid, Signal, bool),
    //  Stopped(Pid, Signal),
    //  PtraceEvent(Pid, Signal, c_int),
    //  PtraceSyscall(Pid),
    //  Continued(Pid),
    //  StillAlive,
    //}

    if let Some(sp) = socket_path {
        if let WaitStatus::Exited(_, i) = child_return {
            event.update_exit_code(i);
        };
        event.update_state(ContainerState::Stopped);
        event.send_event(sp)?;
    }
    update_state(container_id, |s| s.set_stopped())?;
    let log_child_return = format!("{:?}\n", child_return);
    #[allow(unused)]
    write_log_file(&mut log_fd, &log_child_return)?;

    Ok(())
}

pub fn start(container_id: &str) -> Result<(), CoreError> {
    let state = read_state_file(container_id)?;

    if state.get_state() != ContainerState::Created {
        return Err(CoreError::ContainerNotCreated(container_id.to_string()));
    }

    connect_create_process(container_id)?;

    update_state(container_id, |s| s.set_running())?;

    Ok(())
}

pub fn state(container_id: &str) -> Result<State, CoreError> {
    read_state_file(container_id)
}

pub fn kill(container_id: &str) -> Result<(), CoreError> {
    let mut state = read_state_file(container_id)?;

    match (state.get_state(), state.get_pid()) {
        (ContainerState::Stopped, _) => {
            return Err(CoreError::ContainerAlreadyStopped(container_id.to_string()));
        }
        (ContainerState::Running | ContainerState::Created, Some(pid)) => {
            match nix::sys::signal::kill(Pid::from_raw(pid), Signal::SIGKILL) {
                Ok(_) => {}
                Err(nix::errno::Errno::ESRCH) => {}
                Err(e) => {
                    return Err(CoreError::Other(anyhow::anyhow!(e)));
                }
            }
        }
        _ => {
            let e = format!("cannot kill {} in current state", container_id);
            return Err(CoreError::Other(anyhow::anyhow!(e)));
        }
    }

    state.set_stopped();
    write_state_file(container_id, &state)?;

    Ok(())
}

pub fn delete(container_id: &str, force: bool) -> Result<(), CoreError> {
    if force {
        cleanup(container_id, force)?;
        return Ok(());
    }

    let state = read_state_file(container_id)?;

    if state.get_state() != ContainerState::Stopped {
        return Err(CoreError::ContainerNotStopped(container_id.to_string()));
    }

    cleanup(container_id, force)?;

    Ok(())
}

pub fn pause(container_id: &str) -> Result<(), CoreError> {
    let state = read_state_file(container_id)?;
    let cgroup_path = format!("/sys/fs/cgroup/clonebox/{}", container_id);

    if state.get_state() != ContainerState::Running {
        return Err(CoreError::ContainerNotRunning(container_id.to_string()));
    }

    set_cgroup(&cgroup_path, "cgroup", "freeze", "1")?;

    update_state(container_id, |s| s.set_paused())?;

    Ok(())
}

pub fn resume(container_id: &str) -> Result<(), CoreError> {
    let state = read_state_file(container_id)?;
    let cgroup_path = format!("/sys/fs/cgroup/clonebox/{}", container_id);

    if state.get_state() != ContainerState::Paused {
        return Err(CoreError::ContainerNotPaused(container_id.to_string()));
    }

    set_cgroup(&cgroup_path, "cgroup", "freeze", "0")?;

    update_state(container_id, |s| s.set_running())?;

    Ok(())
}

fn enter_namespace(pid: i32, ns: &str, flag: CloneFlags) -> Result<(), CoreError> {
    let path = format!("/proc/{}/ns/{}", pid, ns);
    let fd = File::open(&path).map_err(|e| SystemError::Io(e, path))?;
    setns(fd.as_fd(), flag).map_err(|e| {
        let e = format!("enter {} for {} failed: {}", ns, pid, e);
        NamespaceError::FailedToEnterNamespace(e)
    })?;

    Ok(())
}

pub fn exec(container_id: &str, cmd: Vec<String>) -> Result<(), CoreError> {
    let state = read_state_file(container_id)?;

    if state.get_state() != ContainerState::Running {
        return Err(CoreError::ContainerNotRunning(container_id.to_string()));
    }

    let bundle_path = get_bundle_path(container_id);
    let mut log_fd = open_log_file(&bundle_path)?;

    let child_pid = state
        .get_pid()
        .ok_or(CoreError::ContainerNotRunning(container_id.to_string()))?;
    let parent_pid = getpid().as_raw();

    let parent_mnt_path = format!("/proc/{}/ns/mnt", parent_pid);
    let parent_mnt_fd = File::open(&parent_mnt_path)
        .map_err(|e| SystemError::Io(e, parent_mnt_path.to_string()))?;

    enter_namespace(child_pid, "uts", CloneFlags::CLONE_NEWUTS)?;
    enter_namespace(child_pid, "pid", CloneFlags::CLONE_NEWPID)?;
    enter_namespace(child_pid, "net", CloneFlags::CLONE_NEWNET)?;
    enter_namespace(child_pid, "mnt", CloneFlags::CLONE_NEWNS)?;

    match unsafe { fork() } {
        Ok(ForkResult::Parent { child, .. }) => {
            waitpid(child, None).map_err(SystemError::Wait)?;
        }
        Ok(ForkResult::Child) => {
            container_exec(Some(cmd), None, None, &mut log_fd);
            #[allow(unreachable_code)]
            unsafe {
                libc::_exit(1);
            }
        }
        Err(e) => return Err(SystemError::Wait(e).into()),
    }

    //TODO: write to log file
    setns(parent_mnt_fd.as_fd(), CloneFlags::CLONE_NEWNS).map_err(|e| {
        let e = format!("enter {} for {} failed: {}", "mnt", parent_pid, e);
        let _ = write_log_file(&mut log_fd, &e);
        NamespaceError::FailedToEnterNamespace(e)
    })?;
    enter_namespace(parent_pid, "uts", CloneFlags::CLONE_NEWUTS)?;
    enter_namespace(parent_pid, "pid", CloneFlags::CLONE_NEWPID)?;
    enter_namespace(parent_pid, "net", CloneFlags::CLONE_NEWNET)?;

    Ok(())
}

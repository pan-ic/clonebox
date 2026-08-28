use anyhow::Context;
#[cfg(target_os = "linux")]
use nix::{
    sched::{CloneFlags, setns},
    sys::{signal::Signal, wait::{waitpid, WaitStatus},},
    unistd::{ForkResult, Pid, execve, fork, getpid, write as nix_write},
};
use std::{
    ffi::CString,
    fs::{File, create_dir_all, remove_dir, remove_dir_all, Permissions,},
    os::{ fd::{AsFd, AsRawFd,}, unix::fs::PermissionsExt, },
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

fn cleanup(container_id: &str, force: bool) -> anyhow::Result<()> {
    let cgroup_path = format!("/sys/fs/cgroup/clonebox/{}", container_id);
    let bundle_path = get_bundle_path(container_id);

    if force {
        remove_dir(&cgroup_path).ok();
        remove_dir_all(bundle_path).ok();
        return Ok(());
    }

    remove_dir(&cgroup_path).context("failed to clean cgroups")?;
    remove_dir_all(&bundle_path).context("failed to clean container bundle")?;

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
pub fn create(container_id: &str, config_path: &str, socket_path: Option<&str>) -> anyhow::Result<()> {
    let bundle_path = get_bundle_path(container_id);
    if Path::new(&bundle_path).exists() {
        anyhow::bail!("container {} already exists", container_id);
    }

    let config = Config::load(config_path).context("failed to load config")?;
    if config.get_root_path().is_empty() {
        anyhow::bail!("config.json: root.path is required");
    }
    if config
        .get_process_args()
        .map(|a| a.is_empty())
        .unwrap_or(true)
    {
        anyhow::bail!("config.json: process.args is required")
    }
    create_dir_all(&bundle_path).context("failed to create container bundle")?;
    std::fs::set_permissions(&bundle_path, Permissions::from_mode(0o700))
        .context("create: failed to chmod bundle dir")?;

    let mut log_fd = open_log_file(&bundle_path).context("create: ")?;
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
        event.send_event(sp).context("create: failed to send event to daemon")?;
    }
    write_state_file(container_id, &state).context("failed to write state")?;
    

    //TODO: replace what can be repaced by config parsing
    let new_root = config.get_root_path();
    let empty = vec![];
    let mounts = config.get_mounts().unwrap_or(&empty);

    let (_, child_cgroup_fd) = create_cgroup(container_id).context("failed to create cgroup")?;
    let resources = vec!["cpu", "memory"];
    init_resources(get_root_cgroup_path(), &resources)
        .context("failed to init host resources")?;
    init_resources(get_app_cgroup_path(), &resources)
        .context("failed to init clonebox resources")?;

    let clone_flags: CloneFlags = CloneFlags::CLONE_NEWPID
        | CloneFlags::CLONE_NEWUTS
        | CloneFlags::CLONE_NEWNS
        | CloneFlags::CLONE_NEWNET;
    let signal: Option<Signal> = Some(nix::sys::signal::Signal::SIGCHLD);

    runtime
        .parent_child_pipe()
        .context("failed to create pipe")?;

    let child_pid: Pid = Clone3::new(signal)
        .flags(clone_flags.bits() as u64)
        .cgroup(child_cgroup_fd.as_raw_fd())
        .build()
        .context("clone failure")?;

    if child_pid == Pid::from_raw(0) {
        make_child_private();
        set_child_hostname(&config.get_hostname().unwrap_or("none".to_string()));
        let new_root = pre_pivot_mount(&bundle_path, new_root, mounts);
        do_pivot_root(&new_root);
        post_pivot_mount(mounts);
        do_default_mounts();

        runtime.freeze_child().context("failed to freeze child")?;

        container_exec(
            config.get_process_args(),
            config.get_process_env(),
            Some(config.get_process_cwd()),
            &mut log_fd,
        );
    }

    create_network(container_id, &child_pid).context("failed to create network")?;

    let child_cgroup_path = get_child_cgroup_path(container_id);
    set_cgroup(&child_cgroup_path, "cpu", "max", "100000 100000")?;
    set_cgroup(&child_cgroup_path, "memory", "max", "256M")?;

    if let Some(sp) = socket_path {
        event.update_state(ContainerState::Created);
        event.send_event(sp).context("create: failed to send event to daemon")?;
    }
    update_state(container_id, |s| s.set_created(child_pid.as_raw()))
        .context("failed to update container state")?;

    runtime
        .parent_proc_socket(container_id)
        .context("failed to create unix socket")?;
    runtime
        .unfreeze_child()
        .context("failed to write into pipe")?;

    let child_return = waitpid(child_pid, None).context("waitpid failure")?;
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
        match child_return {
            WaitStatus::Exited(_, i) => {
                event.update_exit_code(i);
            },
            _ => {},
        };
        event.update_state(ContainerState::Stopped);
        event.send_event(sp).context("create: failed to send event to daemon")?;
    }
    update_state(container_id, |s| s.set_stopped()).context("failed to update container state")?;
    let log_child_return = format!("{:?}\n", child_return);
    #[allow(unused)]
    write_log_file(&mut log_fd, &log_child_return);

    Ok(())
}

pub fn start(container_id: &str) -> anyhow::Result<()> {
    let state = read_state_file(container_id).context("failed to read state file")?;

    if state.get_state() != ContainerState::Created {
        anyhow::bail!(
            "{} has not been created or is already running",
            container_id
        );
    }

    connect_create_process(container_id).context("failed to connect to start process")?;
    
    update_state(container_id, |s| s.set_running()).context("failed to update to running state")?;

    Ok(())
}

pub fn state(container_id: &str) -> anyhow::Result<State> {
    Ok(read_state_file(container_id).context("failed to read state file")?)
}

pub fn kill(container_id: &str) -> anyhow::Result<()> {
    let mut state = read_state_file(container_id).context("failed to read state file")?;

    match (state.get_state(), state.get_pid()) {
        (ContainerState::Stopped, _) => anyhow::bail!("{} already stopped", container_id),
        (ContainerState::Running | ContainerState::Created, Some(pid)) => {
            match nix::sys::signal::kill(Pid::from_raw(pid), Signal::SIGKILL) {
                Ok(_) => {}
                Err(nix::errno::Errno::ESRCH) => {}
                Err(e) => return Err(e.into()),
            }
        }
        _ => anyhow::bail!("cannot kill container in current state"),
    }

    state.set_stopped();
    write_state_file(container_id, &state).context("failed to write state file")?;

    Ok(())
}

pub fn delete(container_id: &str, force: bool) -> anyhow::Result<()> {
    if force {
        cleanup(container_id, force).context("delete: failed to force cleanup:")?;
        return Ok(());
    }

    let state = read_state_file(container_id).context("delete: failed to read state file")?;

    if state.get_state() != ContainerState::Stopped {
        anyhow::bail!("{} is used/busy", container_id);
    }

    cleanup(container_id, force).context("delete: failed to cleanup:")?;

    Ok(())
}

pub fn pause(container_id: &str) -> anyhow::Result<()> {
    let state = read_state_file(container_id).context("failed to read state file")?;
    let cgroup_path = format!("/sys/fs/cgroup/clonebox/{}", container_id);

    if state.get_state() != ContainerState::Running {
        anyhow::bail!("{} is not running", container_id);
    }

    set_cgroup(&cgroup_path, "cgroup", "freeze", "1").context("failed to freeze cgroup")?;

    update_state(container_id, |s| s.set_paused()).context("failed to update to paused")?;

    Ok(())
}

pub fn resume(container_id: &str) -> anyhow::Result<()> {
    let state = read_state_file(container_id).context("failed to read state file")?;
    let cgroup_path = format!("/sys/fs/cgroup/clonebox/{}", container_id);

    if state.get_state() != ContainerState::Paused {
        anyhow::bail!("{} is paused", container_id);
    }

    set_cgroup(&cgroup_path, "cgroup", "freeze", "0").context("failed to unfreeze cgroup")?;

    update_state(container_id, |s| s.set_running()).context("failed to update state to running")?;

    Ok(())
}

fn enter_namespace(pid: i32, ns: &str, flag: CloneFlags) -> anyhow::Result<()> {
    let fd = File::open(format!("/proc/{}/ns/{}", pid, ns))?;
    setns(fd.as_fd(), flag)?;
    Ok(())
}

pub fn exec(container_id: &str, cmd: Vec<String>) -> anyhow::Result<()> {
    let state = read_state_file(container_id).context("failed to read state file")?;

    if state.get_state() != ContainerState::Running {
        anyhow::bail!("{} is not running", container_id);
    }

    let bundle_path = get_bundle_path(container_id);
    let mut log_fd = open_log_file(&bundle_path).context("exec: ")?;

    let child_pid = state
        .get_pid()
        .ok_or_else(|| anyhow::anyhow!("container has no pid"))?;
    let parent_pid = getpid().as_raw();

    let parent_mnt_fd = File::open(format!("/proc/{}/ns/mnt", parent_pid))?;

    enter_namespace(child_pid, "uts", CloneFlags::CLONE_NEWUTS)
        .context("enter child uts failed")?;
    enter_namespace(child_pid, "pid", CloneFlags::CLONE_NEWPID)
        .context("enter child pid failed")?;
    enter_namespace(child_pid, "net", CloneFlags::CLONE_NEWNET)
        .context("enter child net failed")?;
    enter_namespace(child_pid, "mnt", CloneFlags::CLONE_NEWNS).context("enter child mnt failed")?;

    match unsafe { fork() } {
        Ok(ForkResult::Parent { child, .. }) => {
            waitpid(child, None).context("waitpid failed")?;
        }
        Ok(ForkResult::Child) => container_exec(Some(cmd), None, None, &mut log_fd),
        Err(_) => anyhow::bail!("failed to fork"),
    }

    //TODO: write to log file
    setns(parent_mnt_fd.as_fd(), CloneFlags::CLONE_NEWNS)?;
    enter_namespace(parent_pid, "uts", CloneFlags::CLONE_NEWUTS)
        .context("enter parent uts failed")?;
    enter_namespace(parent_pid, "pid", CloneFlags::CLONE_NEWPID)
        .context("enter parent pid failed")?;
    enter_namespace(parent_pid, "net", CloneFlags::CLONE_NEWNET)
        .context("enter parent net failed")?;

    Ok(())
}

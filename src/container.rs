use anyhow::Context;
#[cfg(target_os = "linux")]
use nix::{
    sched::{CloneCb, CloneFlags, clone},
    sys::{wait::waitpid, signal::Signal},
    unistd::{Pid, execve, getpid, write as nix_write},
};
use std::{
    ffi::{CString, c_int},
    fs::remove_dir,
    os::fd::{AsRawFd, OwnedFd, RawFd},
};

use crate::clone3::Clone3;

use crate::cgroup::{
    get_root_cgroup_path,
    get_app_cgroup_path,
    get_child_cgroup_path,
    create_cgroup,
    init_resources,
    set_cgroup,
};

use crate::namespace::{
    make_child_private,
    set_child_hostname,
    bind_mount_child,
    do_pivot_root,
    mount_child_proc,
};

use crate::network::create_network;

#[allow(unreachable_code)]
#[cfg(target_os = "linux")]
pub fn create_child_process(name: &str, cmd: &str) -> anyhow::Result<()> {
    //execve() call variables
    let path = CString::new("/bin/sh").context("c_str failure")?;
    let ca = [
        CString::new("sh").context("c_str failure")?,
        CString::new("-c").context("c_str failure")?,
        CString::new(cmd).context("c_str failure")?,
    ];
    let parent_pid = getpid();
    let child_pid: Pid;

    //mount() + pivot_root() call variables
    //TODO: replace what can be repaced by config parsing
    //note that variable might move in the clone call back if not needed elsewhere (and so, won't have
    //to be static), will be determined during refacto
    let new_root: &'static str = "/home/debian/clonebox/alpine_fs";
    let mount_proc: &'static str = "proc";
    let mount_proc_path: &'static str = "/proc";
    let put_old: &'static str = "/put_old";
    let child_old_path: &'static str = "/home/debian/clonebox/alpine_fs/put_old";

    let cb: CloneCb = Box::new(|| -> isize {
        make_child_private();
        set_child_hostname(name);
        bind_mount_child(new_root);
        do_pivot_root(child_old_path, new_root, put_old);
        mount_child_proc(mount_proc, mount_proc_path);

        let Err(e) = execve(&path, &ca, &[] as &[CString]);
        let e = format!("execve failed: {}\n", e);
        //write might fail but we are about to exit anyway
        let _ = nix_write(std::io::stderr(), e.as_bytes());
        unsafe {
            libc::_exit(1);
        }
        0
    });

    //clone() call variables
    let clone_flags: CloneFlags = CloneFlags::CLONE_NEWPID
        | CloneFlags::CLONE_NEWUTS
        | CloneFlags::CLONE_NEWNS
        | CloneFlags::CLONE_NEWNET;
    let signal: Option<Signal> = Some(nix::sys::signal::Signal::SIGCHLD);

    println!("Parent pid is {}", parent_pid);

    let (child_cgroup_path, child_cgroup_fd) = create_cgroup(name)?;

    let os_resources = vec!["cpu", "memory"];
    let app_resources = vec!["cpu", "memory"];

    init_resources(get_root_cgroup_path(), &os_resources);
    init_resources(get_app_cgroup_path(), &app_resources);

    unsafe {
        child_pid = Clone3::new(signal)
            .flags(clone_flags.bits() as u64)
            .cgroup(child_cgroup_fd.as_raw_fd())
            .build(Box::new(cb))
            .context("clone failure")?;
    }    

    create_network(&child_pid)?;

    let child_resources = vec!["cpu", "memory"];
    let child_cgroup_path = get_child_cgroup_path(&name);
    init_resources(&child_cgroup_path, &child_resources);
    set_cgroup(&child_cgroup_path, child_resources[0],"max","100000 100000")?;
    set_cgroup(&child_cgroup_path, child_resources[1], "max", "256M")?;

    println!("Child pid is {}", child_pid);
    let child_return = waitpid(child_pid, None).context("waitpid failure")?;
    println!("Child return is: {:?}", child_return);

    //let _ = remove_dir(child_cgroups).context("failed to clean cgroups")?;

    Ok(())
}

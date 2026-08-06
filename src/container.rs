use anyhow::Context;
#[cfg(target_os = "linux")]
use nix::sched::{CloneCb, CloneFlags, clone, setns};
use nix::{
    mount::{MntFlags, MsFlags, mount, umount2},
    sys::wait::waitpid,
    unistd::{Pid, chdir, execve, getpid, pivot_root, sethostname, write},
};
use std::ffi::{CString, c_int};
use std::fs::{
    create_dir_all,
    File,
    remove_dir,
};
use std::path::Path;
use std::os::fd::AsFd;
use std::net::Ipv4Addr;

use crate::network::{
    add_default_route,
    get_interface_index,
    create_netlink_socket,
    create_veth_pair,
    move_to_netns,
    set_interface_up,
    set_ip_addr,
    Writer,
};

macro_rules! child_try {
    ($expr:expr, $msg:expr, $eval:expr) => {
        if let Err(e) = $expr {
            let msg = format!("{}: {}\n", $msg, e);
            let _ = write(std::io::stderr(), msg.as_bytes());
            unsafe {
                libc::_exit($eval as c_int);
            }
        }
    };
}

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

    //clone() call variables
    let cb: CloneCb = Box::new(|| -> isize {
        //child mount has to be private else it's still shared with the parent
        child_try!(
            mount(
                None::<&str>,
                "/",
                None::<&str>,
                MsFlags::MS_PRIVATE | MsFlags::MS_REC,
                None::<&str>
            ),
            "private mount",
            1
        );

        //change hostname
        child_try!(sethostname(name), "sethostname", 1);

        //bind mount fs
        child_try!(
            mount(
                Some(new_root),
                new_root,
                None::<&str>,
                MsFlags::MS_BIND,
                None::<&str>
            ),
            "bind mount",
            1
        );

        //pivot_root()
        child_try!(
            create_dir_all(Path::new(&child_old_path)),
            "create_dir_all",
            1
        );
        child_try!(pivot_root(new_root, child_old_path), "pivot_root", 1);
        child_try!(chdir("/"), "chdir", 1);
        child_try!(umount2(put_old, MntFlags::MNT_DETACH), "unmount2", 1);
        child_try!(remove_dir(Path::new(put_old)), "remove_dir", 1);

        //mount proc
        child_try!(
            mount(
                Some(mount_proc),
                mount_proc_path,
                Some(mount_proc),
                MsFlags::empty(),
                None::<&str>
            ),
            "fs mount",
            1
        );

        let Err(e) = execve(&path, &ca, &[] as &[CString]);
        let e = format!("execve failed: {}\n", e);
        //write might fail but we are about to exit anyway
        let _ = write(std::io::stderr(), e.as_bytes());
        unsafe {
            libc::_exit(1);
        }
        0
    });

    //clone() call variables
    let mut stack = vec![0u8; 1024 * 1024];
    let clone_flags: CloneFlags = CloneFlags::CLONE_NEWPID
        | CloneFlags::CLONE_NEWUTS
        | CloneFlags::CLONE_NEWNS
        | CloneFlags::CLONE_NEWNET;
    let signal: Option<c_int> = Some(libc::SIGCHLD);

    println!("Parent pid is {}", parent_pid);

    unsafe {
        child_pid = clone(cb, &mut stack, clone_flags, signal).context("clone failure")?;
    }

    let host_ns_fd = File::open("/proc/self/ns/net")?;
    let peer_ns_fd = File::open(format!("/proc/{}/ns/net", child_pid.as_raw()))?;
    let host = "veth1";
    let host_address = Ipv4Addr::new(10, 0, 0, 1);
    let peer_address = Ipv4Addr::new(10, 0, 0, 2);
    let peer = "veth1_peer";
    let mut w = Writer {
        buf : Vec::new(),
    };
    let host_sk = create_netlink_socket()?;
    let _ = create_veth_pair(host_sk.as_fd(), &mut w, host, peer)?;
    let host_i_id = get_interface_index(host_sk.as_fd(), &mut w, host)?;
    let _ = set_ip_addr(host_sk.as_fd(), &mut w, host_i_id, host_address, 24u8)?;
    let _ = set_interface_up(host_sk.as_fd(), &mut w, host_i_id)?;
    let child_i_id = get_interface_index(host_sk.as_fd(), &mut w, peer)?; 
    let _ = move_to_netns(host_sk.as_fd(), &mut w, &child_i_id, &peer_ns_fd)?;

    let _ = setns(peer_ns_fd.as_fd(), CloneFlags::CLONE_NEWNET)?;

    let child_sk = create_netlink_socket()?;
    let _ = set_ip_addr(child_sk.as_fd(), &mut w, child_i_id, peer_address, 24u8)?;
    let _ = set_interface_up(child_sk.as_fd(), &mut w, child_i_id)?;
    let _ = set_interface_up(child_sk.as_fd(), &mut w, 1)?;
    let _ = add_default_route(child_sk.as_fd(), &mut w, host_address)?;

    drop(child_sk);

    let _ = setns(host_ns_fd.as_fd(), CloneFlags::CLONE_NEWNET)?;

    std::fs::write("/proc/sys/net/ipv4/ip_forward", "1")?;
    std::process::Command::new("iptables")
        .args(["-t", "nat", "-A", "POSTROUTING", "-s", "10.0.0.0/24", "-o", "ens2", "-j", "MASQUERADE"])
        .status()?;

    println!("Child pid is {}", child_pid);
    let child_return = waitpid(child_pid, None).context("waitpid failure")?;
    println!("Child return is: {:?}", child_return);

    Ok(())
}

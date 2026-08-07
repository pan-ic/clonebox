use nix::{
    mount::{MntFlags, MsFlags, mount, umount2},
    unistd::{chdir, pivot_root, sethostname, write},
};

use std::ffi::c_int;
use std::fs::{
    create_dir_all,
    remove_dir,
};
use std::path::Path;

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

pub(crate) fn make_child_private() {
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
}

pub(crate) fn set_child_hostname(name: &str) {
    child_try!(sethostname(name), "sethostname", 1);
}

pub(crate) fn bind_mount_child(new_root: &'static str) {
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
}

pub(crate) fn do_pivot_root(
    child_old_path: &'static str,
    new_root: &'static str,
    put_old: &'static str) 
{
    child_try!(
        create_dir_all(Path::new(&child_old_path)),
        "create_dir_all",
        1
    );
    child_try!(pivot_root(new_root, child_old_path), "pivot_root", 1);
    child_try!(chdir("/"), "chdir", 1);
    child_try!(umount2(put_old, MntFlags::MNT_DETACH), "unmount2", 1);
    child_try!(remove_dir(Path::new(put_old)), "remove_dir", 1);
}

pub(crate) fn mount_child_proc(mount_proc: &'static str,
    mount_proc_path: &'static str)
{
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
}

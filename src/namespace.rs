use nix::{
    mount::{MntFlags, MsFlags, mount, umount2},
    unistd::{chdir, pivot_root, sethostname, write},
};

use std::{
    ffi::c_int,
    fs::{
        create_dir_all,
        remove_dir,
    },
};
use std::path::Path;

use crate::config::Mount;

//TODO: inlcude log file fd here + write log file
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

pub(crate) fn pre_pivot_mount(bundle_path: &str,
    new_root: &str, 
    mounts: &Vec<Mount>) -> String {

    mount_child(
        Some(new_root),
        new_root,
        None::<&str>,
        MsFlags::MS_BIND,
        None::<&str>,
        "bind mount"
    );

    for mount in mounts {
        if mount.options.as_ref().map(|o| o.contains(&"bind".to_string())).unwrap_or(false) {
            let options_formatted_string: Option<String> = mount.options.as_ref().map(|o| o.join(","));
            let host_dest = format!("{}{}", new_root, mount.destination);
            mount_child(mount.source.as_deref(),
                &host_dest,
                mount.mount_type.as_deref(),
                MsFlags::MS_BIND,
                options_formatted_string.as_deref(),
                "bind"
            );
            return new_root.to_string();
        }
    };

    let upper_dir = format!("{}/upper", bundle_path);
    let work_dir = format!("{}/work", bundle_path);
    let merged = format!("{}/merged", bundle_path);
    
    child_try!(
        create_dir_all(Path::new(&upper_dir)),
        "pre_pivot: create_dir_all: upper_dir",
        1
    );
    child_try!(
        create_dir_all(Path::new(&work_dir)),
        "pre_pivot: create_dir_all: work_dir",
        1
    );
    child_try!(
        create_dir_all(Path::new(&merged)),
        "create_dir_all",
        1
    );

    let options_formatted_string = format!("lowerdir={},upperdir={},workdir={}",
        new_root,
        upper_dir,
        work_dir);

    mount_child(
        Some("overlay"), 
        &merged,
        Some("overlay"),
        MsFlags::empty(),
        Some(options_formatted_string.as_str()),
        "overlayfs"
    );

    merged
}

pub(crate) fn do_pivot_root(new_root: &str) {
    let put_old = "/put_old";
    let child_old_path = format!("{}/put_old", new_root);

    child_try!(
        create_dir_all(Path::new(&child_old_path)),
        "create_dir_all",
        1
    );
    child_try!(pivot_root(new_root, child_old_path.as_str()), "pivot_root", 1);
    child_try!(chdir("/"), "chdir", 1);
    child_try!(umount2(put_old, MntFlags::MNT_DETACH), "unmount2", 1);
    child_try!(remove_dir(Path::new(put_old)), "remove_dir", 1);
}

pub(crate) fn mount_child(src: Option<&str>, 
    dest: &str,
    mount_type: Option<&str>,
    flags: MsFlags,
    data: Option<&str>,
    msg: &str)
{
    child_try!(
        mount(
            src,
            dest,
            mount_type,
            flags,
            data,
        ),
        msg,
        1
    );
}

pub(crate) fn post_pivot_mount(mounts: &Vec<Mount>) {
    for mount in mounts {
        let (mount_flag, msg) = match mount.mount_type.as_deref() {
            Some("tmpfs") => {
                (MsFlags::empty(), "tmpfs")
            },
            _ => {
                    (MsFlags::empty(), "skip")
            },
        };

        let options_formatted_string: Option<String> = mount.options.as_ref().map(|o| o.join(","));

        if msg != "skip" { 
            mount_child(mount.source.as_deref(),
                &mount.destination,
                mount.mount_type.as_deref(),
                mount_flag,
                options_formatted_string.as_deref(),
                msg
            );
        }
    };
}

pub(crate) fn do_default_mounts() {
    let mount_proc = "proc";
    let mount_proc_path = "/proc";
    let mount_sys = "sysfs";
    let mount_sys_path = "/sys";
    let mount_dev = "devtmpfs";
    let mount_dev_path = "/dev";

    mount_child(Some(mount_proc), 
        mount_proc_path, 
        Some(mount_proc), 
        MsFlags::empty(), 
        None::<&str>,
        mount_proc
    );
    mount_child(Some(mount_sys), 
        mount_sys_path, 
        Some(mount_sys), 
        MsFlags::empty(), 
        None::<&str>,
        mount_sys
    ); 
    mount_child(Some(mount_dev), 
        mount_dev_path, 
        Some(mount_dev), 
        MsFlags::empty(), 
        None::<&str>,
        mount_dev
    );
}

use anyhow::Context;
use clap::{Parser, Subcommand};
#[cfg(target_os = "linux")]
use nix::sched::{CloneCb, CloneFlags, clone};
use nix::{
    mount::{MntFlags, MsFlags, mount, umount2},
    sys::wait::waitpid,
    unistd::{Pid, chdir, execve, getpid, pivot_root, sethostname, write},
};
use std::ffi::{CString, c_int};
use std::fs::{create_dir_all, remove_dir};
use std::path::Path;
//use core::ffi::c_str;

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

#[derive(Debug, Parser)]
#[command(version, author, about)]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(arg_required_else_help = true)]
    Run {
        #[arg(long, default_value = "/run/clonebox")]
        config: Option<String>,
        #[arg(long, required = true)]
        name: String,
        //temporary solution adopted for manual tests, will be replaced soon by config parsing
        #[arg(long, required = true)]
        cmd: String,
    },
    /*
    #[command(arg_required_else_help = true)]
    Start {}
    */
}

#[allow(unreachable_code)]
#[cfg(target_os = "linux")]
fn create_child_process(name: &str, cmd: &str) -> anyhow::Result<()> {
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
    let clone_flags: CloneFlags =
        CloneFlags::CLONE_NEWPID | CloneFlags::CLONE_NEWUTS | CloneFlags::CLONE_NEWNS;
    let signal: Option<c_int> = Some(libc::SIGCHLD);

    println!("Parent pid is {}", parent_pid);

    unsafe {
        child_pid = clone(cb, &mut stack, clone_flags, signal).context("clone failure")?;
    }

    println!("Child pid is {}", child_pid);
    let child_return = waitpid(child_pid, None).context("waitpid failure")?;
    println!("Child return is: {:?}", child_return);

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
        Commands::Run { name, cmd, .. } => {
            println!("Container {} starts", name);
            create_child_process(&name, &cmd)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parse() {
        let cli =
            Cli::try_parse_from(["clonebox", "run", "--name", "test", "--cmd", "echo OK"]).unwrap();

        match cli.cmd {
            Commands::Run { config, name, cmd } => {
                assert_eq!(config, Some(String::from("/run/clonebox")));
                assert_eq!(name, "test");
                assert_eq!(cmd, "echo OK");
            }
        };
    }
}

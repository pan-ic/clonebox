use anyhow::Context;
use clap::{Parser, Subcommand};
#[cfg(target_os = "linux")]
use nix::sched::{CloneCb, CloneFlags, clone};
use nix::{
    sys::wait::waitpid,
    unistd::{Pid, execve, getpid, sethostname, write},
};
use std::ffi::{CString, c_int};
//use core::ffi::c_str;

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
    let path = CString::new("/bin/sh").context("c_str failure")?;
    let ca = [
        CString::new("sh").context("c_str failure")?,
        CString::new("-c").context("c_str failure")?,
        CString::new(cmd).context("c_str failure")?,
    ];
    let parent_pid = getpid();
    let child_pid: Pid;
    let cb: CloneCb = Box::new(|| -> isize {
        if let Err(e) = sethostname(name) {
            let e = format!("sethostname failed: {}", e);
            let _ = write(std::io::stderr(), e.as_bytes());
            unsafe {
                libc::_exit(1);
            }
        };

        let Err(e) = execve(&path, &ca, &[] as &[CString]);
        let e = format!("execve failed: {}\n", e);
        //write might fail but we are about to exit anyway
        let _ = write(std::io::stderr(), e.as_bytes());
        unsafe {
            libc::_exit(1);
        }
        0
    });
    let mut stack = vec![0u8; 1024 * 1024];
    let clone_flags: CloneFlags = CloneFlags::CLONE_NEWPID | CloneFlags::CLONE_NEWUTS;
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

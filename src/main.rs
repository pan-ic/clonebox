use clap::{Parser, Subcommand};
use libc;
use nix::{
    sched::{clone, CloneCb, CloneFlags},
    sys::wait::waitpid,
    unistd::{execve, getpid, Pid, write},
};
use std::ffi::{CString, c_int};
//use core::ffi::c_str;

#[derive(Parser)]
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
fn create_child_process(cmd: &str) {
    let path = CString::new("/bin/sh").expect("c_str failed");
    let ca = [
        CString::new("sh").expect("c_str failed"),
        CString::new("-c").expect("c_str failed"),
        CString::new(cmd).expect("c_str failed"),
    ];
    let parent_pid = getpid();
    let child_pid: Pid;
    let cb: CloneCb = Box::new(|| -> isize {
        let Err(e) = execve(&path, &ca, &[] as &[CString]);
        let e = format!("execve failed: {}\n", e);
        write(std::io::stderr(), e.as_bytes()).unwrap();
        unsafe { libc::_exit(0); } 
        0
    });
    let mut stack = vec![0u8; 1024 * 1024];
    let clone_flags: CloneFlags = CloneFlags::CLONE_NEWPID;
    let signal: Option<c_int> = Some(libc::SIGCHLD);

    println!("Parent pid is {}", parent_pid);
    
    unsafe { child_pid = clone(cb, &mut stack, clone_flags, signal).unwrap(); }
    
    println!("Child pid is {}", child_pid);
    waitpid(child_pid, None).unwrap();
}

fn main() {
    let cli = Cli::parse();

    match cli.cmd {
        Commands::Run { name, cmd, .. } => {
            println!("Container {} starts", name);
            create_child_process(&cmd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn creates_child_process() {
        todo!()
    }
}

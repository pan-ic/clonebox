use clap::{Parser, Subcommand};
use nix::{
    sys::wait::waitpid,
    unistd::{ForkResult, execve, fork, getpid, write},
};
use std::ffi::CString;

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

fn create_child_process(cmd: &str) {
    match unsafe { fork() } {
        Ok(ForkResult::Parent { child, .. }) => {
            println!("Parent pid is {}", getpid());
            println!("Child pid is {}", child);
            waitpid(child, None).unwrap();
            println!("Back to parent");
        }
        Ok(ForkResult::Child) => {
            let path = CString::new("/bin/sh").expect("c_str failed");
            let ca = [
                CString::new("sh").expect("c_str failed"),
                CString::new("-c").expect("c_str failed"),
                CString::new(cmd).expect("c_str failed"),
            ];

            let Err(e) = execve(&path, &ca, &[] as &[CString]);
            let e = format!("execve failed: {}\n", e);
            write(std::io::stderr(), e.as_bytes()).unwrap();
            unsafe { libc::_exit(0) };
        }
        Err(_) => panic!("Fork failed"),
    }
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

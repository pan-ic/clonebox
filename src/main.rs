use clap::{Command, arg};
use nix::{
    sys::wait::waitpid,
    unistd::{ForkResult, execve, fork, getpid, write},
};
use std::ffi::CString;

fn create_child_process() {
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
                CString::new("sleep 120").expect("c_str failed"),
            ];

            write(std::io::stdout(), b"Child starts\n").unwrap();
            let _ = execve(&path, &ca, &[] as &[CString]);
            write(std::io::stdout(), b"Child ends\n").unwrap();
            unsafe { libc::_exit(0) };
        }
        Err(_) => panic!("Fork failed"),
    }
}

fn cli() -> Command {
    Command::new("clonebox")
        .bin_name("clonebox")
        .about("Linux subset of the OCI runtime specification")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .subcommand(
            Command::new("run")
                .about("Start container")
                .arg(arg!(<CONTAINER_NAME> "The name of the container to run").last(true))
                .arg_required_else_help(true)
                .arg(arg!(-b <BUNDLE_PATH> "The path of the bundle")),
        )
}

fn main() {
    let arg_matches = cli().get_matches();

    match arg_matches.subcommand() {
        Some(("run", sub)) => {
            create_child_process();
            println!(
                "Container {} starts",
                sub.get_one::<String>("CONTAINER_NAME").expect("required")
            );
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_child_process() {
        todo!()
    }

    #[test]
    fn core_pipeline() {
        todo!()
    }
}

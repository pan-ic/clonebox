#[cfg(target_os = "linux")]
mod integration_test {
    use std::path::Path;
    use std::process::Command;

    #[allow(unused)]
    fn build() {
        let build = Command::new("cargo").arg("build").status().unwrap();
    }

    fn get_binary_path<'a>() -> &'a Path {
        let exec_path = Path::new("./target/debug/clonebox");

        if !(exec_path.exists()) {
            build();
        };

        exec_path
    }

    #[test]
    fn binary_exit_cleanly() {
        let exit = Command::new(get_binary_path())
            .args(["run", "--name", "test", "--cmd", "echo OK"])
            .status()
            .unwrap();

        assert!(exit.success());
    }

    //requires state management to check pid in /proc/{pid}/ns/pid
    #[test]
    fn child_process_own_inode() {
        todo!();
    }

    //requires state management to compare both pid, proves that clone() works
    #[test]
    fn child_process_own_pid() {
        todo!();
    }

    //not yet, after config parsing
    /*
    #[test]
    fn execve_error_on_failure() {
        let run = Command::new(get_binary_path())
            .args(["run", "--name", "test", "--cmd", "im_sure_that_command_does_not_exists"])
            .output()
            .unwrap();

        println!("{}", String::from_utf8_lossy(&run.stdout));
        println!("status value: {}", run.status);
        println!("wo neg: {}, w neg: {}", run.status.success(), !run.status.success());
        assert!(!run.status.success());
    }
    */
}

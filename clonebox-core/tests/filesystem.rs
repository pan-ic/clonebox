mod common;

#[cfg(target_os = "linux")]
mod filesystem {
    use super::common::{cleanup, get_binary_path};
    use std::process::Command;

    #[test]
    fn bind_mount_file_appears_on_host() {
        let id = "bind_mount_filesystem_test";
        cleanup(id);

        // create + start container
        // exec: touch /data/testfile
        // assert file exists at /home/bind/testfile on host
        Command::new(&get_binary_path())
            .args(["create", id, "tests/filesystem_config"])
            .spawn()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(500));

        Command::new(&get_binary_path())
            .args(["start", id])
            .spawn()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(1500));

        Command::new(&get_binary_path())
            .args(["exec", id, "/bin/touch /data/testfile"])
            .spawn()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(1500));

        assert!(std::path::Path::new("/home/bind/testfile").exists());

        cleanup(id);
    }

    #[test]
    fn proc_mounted_on_child() {
        let id = "proc_mounted_on_child";
        cleanup(id);

        Command::new(&get_binary_path())
            .args(["create", id, "tests/filesystem_config"])
            .spawn()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(500));

        Command::new(&get_binary_path())
            .args(["start", id])
            .spawn()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(1500));

        let command = Command::new(&get_binary_path())
            .args(["exec", id, "/bin/ls /proc/self"])
            .output()
            .unwrap();

        assert!(
            String::from_utf8_lossy(&command.stdout).contains("fd")
                || String::from_utf8_lossy(&command.stderr).len() == 0
        );

        cleanup(id);
    }
}

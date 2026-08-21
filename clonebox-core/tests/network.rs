mod common;

#[cfg(target_os = "linux")]
mod network {
    use super::common::{cleanup, get_binary_path};
    use std::process::Command;

    #[test]
    fn container_ping() {
        let id = "container_ping";
        cleanup(id);

        Command::new(&get_binary_path())
            .args(["create", id, "tests/network_config"])
            .spawn()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(500));

        Command::new(&get_binary_path())
            .args(["start", id])
            .spawn()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(1500));

        let command = Command::new(&get_binary_path())
            .args(["exec", id, "/bin/ping -c 3 8.8.8.8"])
            .status()
            .unwrap();

        assert!(command.success());

        cleanup(id);
    }
}

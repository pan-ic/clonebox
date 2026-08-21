mod common;

#[cfg(target_os = "linux")]
mod lifecycle_test {
    use super::common::{cleanup, get_binary_path, get_state_file_path, path_resolver};
    use std::fs::read_to_string;
    use std::process::Command;

    #[test]
    fn lifecycle_happy_path() {
        let id = "happy_path";
        cleanup(id);

        let test_file = path_resolver("lifecycle_config");

        Command::new(&get_binary_path())
            .args(["create", id, &test_file])
            .spawn()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(500));
        let output = read_to_string(&get_state_file_path(id)).unwrap();

        assert!(output.contains("\"status\":\"created\""));

        Command::new(&get_binary_path())
            .args(["start", id])
            .spawn()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(500));
        let output = read_to_string(&get_state_file_path(id)).unwrap();

        assert!(output.contains("\"status\":\"running\""));

        Command::new(&get_binary_path())
            .args(["pause", id])
            .spawn()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(500));
        let output = read_to_string(&get_state_file_path(id)).unwrap();

        assert!(output.contains("\"status\":\"paused\""));

        Command::new(&get_binary_path())
            .args(["resume", id])
            .spawn()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(500));
        let output = read_to_string(&get_state_file_path(id)).unwrap();

        assert!(output.contains("\"status\":\"running\""));

        let command = Command::new(&get_binary_path())
            .args(["exec", id, "/bin/echo OK"])
            .status()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(500));

        assert!(command.success());

        Command::new(&get_binary_path())
            .args(["kill", id])
            .spawn()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(500));
        let output = read_to_string(&get_state_file_path(id)).unwrap();

        assert!(output.contains("\"status\":\"stopped\""));

        let command = Command::new(&get_binary_path())
            .args(["delete", id])
            .status()
            .unwrap();

        assert!(command.success());
    }

    #[test]
    fn natural_direct_exit_success_test() {
        let id = "natural_direct_exit_success_test";
        cleanup(id);

        let _create = Command::new(&get_binary_path())
            .args(["create", id, "tests/lifecycle_config"])
            .spawn()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(500));

        Command::new(&get_binary_path())
            .args(["start", id])
            .spawn()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_secs(35));

        let output = read_to_string(&get_state_file_path(id)).unwrap();
        assert!(output.contains("\"status\":\"stopped\""));

        cleanup(id);
    }
}

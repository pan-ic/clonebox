mod common;

#[cfg(target_os = "linux")]
mod error_cases {
    use super::common::{cleanup, get_binary_path};
    use std::process::Command;

    #[test]
    fn duplicated_id() {
        let id = "duplicated_id";
        cleanup(id);

        Command::new(&get_binary_path())
            .args(["create", id, "tests/error_cases/config"])
            .spawn()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(500));

        let create = Command::new(&get_binary_path())
            .args(["create", id, "tests/error_cases/config"])
            .status()
            .unwrap();

        assert!(!create.success());

        cleanup(id);
    }

    #[test]
    fn kill_non_existent() {
        let id = "kill_non_existent";

        let kill = Command::new(&get_binary_path())
            .args(["kill", id])
            .status()
            .unwrap();

        assert!(!kill.success());
    }

    #[test]
    fn delete_non_existent() {
        let id = "delete_non_existent";

        let delete = Command::new(&get_binary_path())
            .args(["delete", id])
            .status()
            .unwrap();

        assert!(!delete.success());
    }

    #[test]
    fn kill_already_stopped() {
        let id = "kill_already_stopped";
        cleanup(id);

        Command::new(&get_binary_path())
            .args(["create", id, "tests/error_cases/config"])
            .spawn()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(500));

        Command::new(&get_binary_path())
            .args(["start", id])
            .spawn()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(30));

        Command::new(&get_binary_path())
            .args(["kill", id])
            .spawn()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(500));

        let kill = Command::new(&get_binary_path())
            .args(["kill", id])
            .status()
            .unwrap();

        assert!(!kill.success());

        cleanup(id);
    }

    #[test]
    fn exec_on_non_running() {
        let id = "exec_on_non_running";

        let command = Command::new(&get_binary_path())
            .args(["exec", id, "/bin/echo OK"])
            .status()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(500));

        assert!(!command.success());
    }

    #[test]
    fn delete_running() {
        let id = "delete_running_success_test";

        //create → start → kill → delete → state returns "not found"
        let command = Command::new(&get_binary_path())
            .args(["delete", id])
            .status()
            .unwrap();

        assert!(!command.success());

        cleanup(id)
    }

    #[test]
    fn state_non_existent() {
        let id = "state_inexisting_success_test";

        let command = Command::new(&get_binary_path())
            .args(["state", id])
            .status()
            .unwrap();

        assert!(!command.success());

        cleanup(id)
    }
}

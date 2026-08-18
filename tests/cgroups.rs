mod common;

#[cfg(target_os = "linux")]
mod cgroups {
    use super::common::{cleanup, get_binary_path};
    use std::process::Command;

    #[test]
    fn cgroup_limits_applied() {
        let id = "cgroup_limits";
        cleanup(id);

        Command::new(&get_binary_path())
            .args(["create", id, "tests/cgroups_config"])
            .spawn()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(500));

        Command::new(&get_binary_path())
            .args(["start", id])
            .spawn()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(500));

        let memory =
            std::fs::read_to_string(format!("/sys/fs/cgroup/clonebox/{}/memory.max", id)).unwrap();

        let cpu =
            std::fs::read_to_string(format!("/sys/fs/cgroup/clonebox/{}/cpu.max", id)).unwrap();

        assert!(memory.trim() == "268435456"); // 256M in bytes
        assert!(cpu.trim() == "100000 100000");

        cleanup(id);
    }
}

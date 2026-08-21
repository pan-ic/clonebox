use std::path::Path;

pub(crate) fn path_resolver(test_file: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/{}", test_file))
        .to_string_lossy()
        .into_owned()
}

pub(crate) fn get_binary_path() -> String {
    String::from("./target/debug/clonebox")
}

#[allow(unused)]
pub(crate) fn get_state_file_path(id: &str) -> String {
    format!("/run/clonebox/{}/state.json", id)
}

pub(crate) fn cleanup(id: &str) {
    if let Ok(state) = std::fs::read_to_string(format!("/run/clonebox/{}/state.json", id)) {
        if let Some(pid_str) = state
            .split("\"pid\":")
            .nth(1)
            .and_then(|s| s.split([',', '}']).next())
        {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                if pid > 0 {
                    unsafe {
                        libc::kill(pid, libc::SIGKILL);
                    }
                }
            }
        }
    }
    std::process::Command::new("pkill")
        .args(["-f", &format!("clonebox create {}", id)])
        .output()
        .ok();
    std::thread::sleep(std::time::Duration::from_millis(200));
    std::fs::remove_dir_all(format!("/run/clonebox/{}", id)).ok();
    std::fs::remove_dir(format!("/sys/fs/cgroup/clonebox/{}", id)).ok();

    std::process::Command::new("ip")
        .args(["link", "delete", "veth1"])
        .output()
        .ok();
}

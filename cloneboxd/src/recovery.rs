use std::{
    collections::HashMap,
    fs::{read_dir, read_to_string},
    path::Path,
    sync::{Arc, Mutex},
};

use tokio::sync::oneshot;

use crate::entry::ContainerEntry;

use clonebox_core::state::{ContainerState, State};

pub fn recover(
    containers: &Arc<Mutex<HashMap<String, ContainerEntry>>>,
    app_data: &str,
) -> anyhow::Result<()> {
    let dir_content = read_dir(app_data)?;

    for entry in dir_content {
        let Ok(entry) = entry else { continue };
        let Ok(dir) = entry.file_type() else { continue };
        if !dir.is_dir() {
            continue;
        };
        let Ok(file_name) = entry.file_name().into_string() else {
            continue;
        };

        let state_path = format!("{}/{}/state.json", app_data, file_name);
        let Ok(buf) = read_to_string(&state_path) else {
            continue;
        };
        let Ok(mut state): Result<State, _> = serde_json::from_str(&buf) else {
            continue;
        };
        if let Some(pid) = state.get_pid() {
            if !Path::new(&format!("/proc/{}", pid)).exists() {
                state.set_state(ContainerState::Stopped);
            }
        } else if state.get_state() == ContainerState::Running {
            state.set_state(ContainerState::Stopped);
        }
        {
            let mut container = containers
                .lock()
                .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
            container
                .entry(file_name.to_string())
                .or_insert(ContainerEntry::new(
                    state.get_state(),
                    None,
                    Vec::<oneshot::Sender<i32>>::new(),
                ));
        }
    }

    Ok(())
}

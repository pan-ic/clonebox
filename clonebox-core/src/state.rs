use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    fs,
};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ContainerState {
    Creating,
    Created,
    Running,
    Stopped,
    Paused,
}

use crate::error::{CoreError, StateError};

impl Display for ContainerState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainerState::Creating => write!(f, "creating"),
            ContainerState::Created => write!(f, "created"),
            ContainerState::Running => write!(f, "running"),
            ContainerState::Stopped => write!(f, "stopped"),
            ContainerState::Paused => write!(f, "paused"),
        }?;

        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct State {
    #[serde(rename = "ociVersion")]
    oci_version: String,
    id: String,
    status: ContainerState,
    pid: Option<i32>,
    bundle: String,
    annotations: Option<HashMap<String, String>>,
}

impl State {
    pub(crate) fn new(
        oci_version: String,
        id: String,
        status: ContainerState,
        pid: Option<i32>,
        bundle: String,
        annotations: Option<HashMap<String, String>>,
    ) -> Self {
        State {
            oci_version,
            id,
            status,
            pid,
            bundle,
            annotations,
        }
    }

    pub fn get_pid(&self) -> Option<i32> {
        self.pid
    }

    #[allow(unused)]
    pub(crate) fn set_pid(&mut self, pid: i32) {
        self.pid = Some(pid);
    }

    pub fn get_state(&self) -> ContainerState {
        self.status
    }

    pub fn set_state(&mut self, status: ContainerState) {
        self.status = status;
    }

    pub fn get_oci_version(&self) -> &str {
        &self.oci_version
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_bundle(&self) -> &str {
        &self.bundle
    }

    pub fn get_annotations(&self) -> &Option<HashMap<String, String>> {
        &self.annotations
    }

    pub(crate) fn set_running(&mut self) {
        self.status = ContainerState::Running;
    }

    pub(crate) fn set_created(&mut self, pid: i32) {
        self.status = ContainerState::Created;
        self.pid = Some(pid);
    }

    pub(crate) fn set_stopped(&mut self) {
        self.status = ContainerState::Stopped;
        self.pid = None;
    }

    pub(crate) fn set_paused(&mut self) {
        self.status = ContainerState::Paused;
    }
}

impl Display for State {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "oci-version: {},\nid: {},\nstatus: {},\n",
            self.oci_version, self.id, self.status
        )?;
        if let Some(pid) = self.pid {
            writeln!(f, "pid: {},", pid)?;
        } else {
            writeln!(f, "pid: null,")?;
        }
        writeln!(f, "bundle: {},", self.bundle)?;
        if let Some(annotations) = &self.annotations {
            writeln!(f, "annotations:")?;
            for (k, v) in annotations {
                writeln!(f, "  {}: {}", k, v)?;
            }
        } else {
            writeln!(f, "annotations: null")?;
        }

        Ok(())
    }
}

pub(crate) fn get_bundle_path(container_id: &str) -> String {
    format!("/run/clonebox/{}", container_id)
}

pub(crate) fn get_state_path(container_id: &str) -> String {
    let bundle_path = get_bundle_path(container_id);

    format!("{}/state.json", bundle_path)
}

pub(crate) fn write_state_file(container_id: &str, state: &State) -> Result<(), CoreError> {
    let path = get_state_path(container_id);
    let ser_json = serde_json::to_string(state).map_err(StateError::Json)?;

    fs::write(path, ser_json).map_err(StateError::Io)?;
    Ok(())
}

pub(crate) fn read_state_file(container_id: &str) -> Result<State, CoreError> {
    let path = get_state_path(container_id);
    match fs::read_to_string(&path) {
        Ok(buf) => Ok(serde_json::from_str(&buf).map_err(StateError::Json)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(CoreError::ContainerNotFound(container_id.to_string()))
        }
        Err(e) => Err(StateError::Io(e).into()),
    }
}

pub(crate) fn update_state<F>(container_id: &str, f: F) -> Result<State, CoreError>
where
    F: FnOnce(&mut State),
{
    let mut state = read_state_file(container_id)?;
    f(&mut state);
    write_state_file(container_id, &state)?;
    Ok(state)
}

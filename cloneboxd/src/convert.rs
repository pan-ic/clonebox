use tonic::Status;

use crate::clonebox_tasks::{ContainerState as ContainerStateProto, State as ProtoState};
use clonebox_core::state::{ContainerState, State};

use clonebox_core::error::CoreError;

pub(crate) fn to_status(e: CoreError) -> Status {
    match e {
        CoreError::ContainerAlreadyExists(id) => {
            let msg = format!("{} already exists", id);
            Status::already_exists(&msg)
        }
        CoreError::ContainerAlreadyStopped(id) => {
            let msg = format!("{} already stopped", id);
            Status::failed_precondition(&msg)
        }
        CoreError::ContainerNotFound(id) => {
            let msg = format!("{} not found", id);
            Status::not_found(&msg)
        }
        CoreError::ContainerNotCreated(id) => {
            let msg = format!("{} not created", id);
            Status::failed_precondition(&msg)
        }
        CoreError::ContainerNotRunning(id) => {
            let msg = format!("{} not running", id);
            Status::failed_precondition(&msg)
        }
        CoreError::ContainerNotPaused(id) => {
            let msg = format!("{} not paused", id);
            Status::failed_precondition(&msg)
        }
        CoreError::ContainerNotStopped(id) => {
            let msg = format!("{} not stopped", id);
            Status::failed_precondition(&msg)
        }
        CoreError::CleanupFailure(err, id) => {
            let msg = format!("failed to clean {} up: {}", id, err);
            Status::internal(&msg)
        }
        CoreError::ConfigError(err) => {
            let msg = format!("config error: {}", err);
            Status::internal(&msg)
        }
        CoreError::StateError(err) => {
            let msg = format!("state error: {}", err);
            Status::internal(&msg)
        }
        CoreError::SystemError(err) => {
            let msg = format!("system error: {}", err);
            Status::internal(&msg)
        }
        CoreError::Other(err) => {
            let msg = format!("Other: {}", err);
            Status::unknown(&msg)
        }
    }
}

impl From<State> for ProtoState {
    fn from(s: State) -> ProtoState {
        let oci_version = String::from(s.get_oci_version());
        let id = String::from(s.get_id());
        let status = match s.get_state() {
            ContainerState::Creating => ContainerStateProto::Creating as i32,
            ContainerState::Created => ContainerStateProto::Created as i32,
            ContainerState::Running => ContainerStateProto::Running as i32,
            ContainerState::Stopped => ContainerStateProto::Stopped as i32,
            ContainerState::Paused => ContainerStateProto::Paused as i32,
        };
        let pid = s.get_pid();
        let bundle = String::from(s.get_bundle());
        let annotations = s.get_annotations().clone().unwrap_or_default();

        ProtoState {
            oci_version,
            id,
            status,
            pid,
            bundle,
            annotations,
        }
    }
}

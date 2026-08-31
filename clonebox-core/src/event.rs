use serde::{Deserialize, Serialize};
use std::{io::Write, os::unix::net::UnixStream};

use crate::state::ContainerState;

use crate::error::{CoreError, EventError};

#[derive(Serialize, Deserialize)]
pub struct Event {
    id: String,
    state: ContainerState,
    exit_code: Option<i32>,
}

impl Event {
    pub fn new(id: String, state: ContainerState, exit_code: Option<i32>) -> Self {
        Event {
            id,
            state,
            exit_code,
        }
    }

    fn get_stream_sk(&self, path: &str) -> Result<UnixStream, CoreError> {
        Ok(UnixStream::connect(path)
            .map_err(|e| EventError::ConnectFailure(e, path.to_string()))?)
    }

    pub fn send_event(&self, path: &str) -> Result<(), CoreError> {
        let mut sk = self.get_stream_sk(path)?;
        let serialized = format!(
            "{}\n",
            serde_json::to_string(&self).map_err(EventError::Json)?
        );

        sk.write_all(serialized.as_bytes())
            .map_err(EventError::WriteFailure)?;

        Ok(())
    }

    pub(crate) fn update_state(&mut self, state: ContainerState) {
        self.state = state;
    }

    pub fn get_state(&self) -> ContainerState {
        self.state
    }

    pub(crate) fn update_exit_code(&mut self, exit_code: i32) {
        self.exit_code = Some(exit_code);
    }

    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    pub fn get_exit_code(&self) -> Option<i32> {
        self.exit_code
    }
}

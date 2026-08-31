use tokio::sync::oneshot;

use clonebox_core::state::ContainerState as ContainerStateCore;
use std::{
    collections::HashMap,
    env,
    env::current_exe,
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Debug)]
pub(crate) struct ContainerEntry {
    pub(crate) state: ContainerStateCore,
    pub(crate) created_waiter: Option<oneshot::Sender<()>>,
    pub(crate) exit_waiter: Vec<oneshot::Sender<i32>>,
}

impl ContainerEntry {
    pub(crate) fn new(
        state: ContainerStateCore,
        created_waiter: Option<oneshot::Sender<()>>,
        exit_waiter: Vec<oneshot::Sender<i32>>,
    ) -> ContainerEntry {
        Self {
            state,
            created_waiter,
            exit_waiter,
        }
    }
}

pub(crate) struct Cloneboxd {
    pub(crate) containers: Arc<Mutex<HashMap<String, ContainerEntry>>>,
}

impl Cloneboxd {
    pub(crate) fn new() -> Cloneboxd {
        Self {
            containers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) fn shim_path() -> anyhow::Result<PathBuf> {
        if let Ok(p) = env::var("CLONEBOX_SHIM") {
            return Ok(PathBuf::from(p));
        };

        Ok(current_exe()?
            .parent()
            .expect("clonebox supposed to have parent")
            .join("clonebox-shim"))
    }
}

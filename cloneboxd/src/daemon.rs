use anyhow::Context;
use tokio::{
    net::UnixListener,
    process::Command,
    sync::oneshot,
};
use tonic::{transport::Server, Request, Response, Status};
use clonebox_core::container::{delete, exec, kill, pause, resume, start, state};
use clonebox_core::state::{ContainerState, State};
use clonebox_tasks::clonebox_tasks_server::{CloneboxTasks, CloneboxTasksServer};
use clonebox_tasks::{
    ContainerState as ContainerStateProto,
    CreateRequest,
    CreateResponse,
    DeleteRequest,
    DeleteResponse,
    ExecRequest,
    ExecResponse,
    KillRequest,
    KillResponse,
    ListRequest,
    ListResponse,
    PauseRequest,
    PauseResponse,
    ResumeRequest,
    ResumeResponse,
    StartRequest,
    StartResponse,
    State as ProtoState,
    StateRequest,
    StateResponse,
    WaitRequest,
    WaitResponse,
};
use std::{
    collections::HashMap,
    env,
    env::current_exe,
    path::PathBuf,
    process::Stdio,
    sync::{Arc, Mutex,},
};
use nix::unistd::setsid;
use tracing::{debug, error, info, warn};

use crate::event::{
    get_app_data_path,
    get_listen_sk,
    event_loop,
};

use clonebox_core::state::ContainerState as ContainerStateCore;

pub mod clonebox_tasks {
    tonic::include_proto!("clonebox_tasks");
}

#[derive(Debug)]
pub(crate) struct ContainerEntry {
    pub(crate) state: ContainerStateCore,
    pub(crate) created_waiter: Option<oneshot::Sender<()>>,
    pub(crate) exit_waiter: Vec<oneshot::Sender<i32>>,
}

impl ContainerEntry {
    pub(crate) fn new(state: ContainerStateCore,
        created_waiter: Option<oneshot::Sender<()>>,
        exit_waiter: Vec<oneshot::Sender<i32>>) -> ContainerEntry {
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

    fn shim_path() -> anyhow::Result<PathBuf> {
        if let Ok(p) = env::var("CLONEBOX_SHIM") {
            return Ok(PathBuf::from(p));
        };

        Ok(current_exe()?
            .parent()
            .expect("clonebox supposed to have parent")
            .join("clonebox-shim"))
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

#[tonic::async_trait]
impl CloneboxTasks for Cloneboxd {
    async fn create(&self, request: Request<CreateRequest>) -> anyhow::Result<Response<CreateResponse>, Status> {
        debug!("Request: {:#?}", request);
        let path_to_exec = Cloneboxd::shim_path()
            .map_err(|e| Status::internal(e.to_string()))?;
        let args = request.into_inner();
        info!("Create querry for {}, using {} config_path", args.container_id, args.config_path);
        let (created_tx, created_rx) = oneshot::channel::<()>();
        let empty_vec = Vec::<oneshot::Sender<i32>>::new();

        let new_entry = ContainerEntry::new(
            ContainerStateCore::Creating,
            Some(created_tx),
            empty_vec,
        );

        {
            let mut containers = self.containers.lock().unwrap();
            containers.entry(args.container_id.clone()).or_insert(new_entry);
        }

        let mut cmd = Command::new(path_to_exec);
        cmd.args([&args.container_id, &args.config_path, &get_app_data_path()])

        cmd.spawn()
            .map_err(|e| {Status::internal(e.to_string())})?;

        match created_rx.await {
            Ok(()) => {
                info!("{} successfully created", args.container_id);
                Ok(Response::new(CreateResponse {}))
            },
            Err(_) => {
                let e = "container creation failed".to_string();
                error!("Create: {:#?}", e);
                Err(Status::internal(e))
            },
        }
    }

    async fn start(&self, request: Request<StartRequest>) -> anyhow::Result<Response<StartResponse>, Status> {
        debug!("Request: {:#?}", request);
        let args = request.into_inner();
        info!("State querry for {}", args.container_id);
       
        //start(&args.container_id).inspect_err(|e| error!(id = %args.container_id, %e, "start failed"))?;
        match start(&args.container_id) {
            Ok(_) => {
                {
                    let mut containers = self.containers.lock().unwrap();
                    if let Some(entry) = containers.get_mut(&args.container_id) {
                        entry.state = ContainerStateCore::Running;
                    }
                }
                info!("{} successfully started", args.container_id);
                Ok(Response::new(StartResponse {}))
            },
            Err(e) => {
                let err = e.to_string();
                error!("Start: {:#?}", err);
                Err(Status::internal(err))
            },
        }
    }

    async fn kill(&self, request: Request<KillRequest>) -> anyhow::Result<Response<KillResponse>, Status> {
        debug!("Request: {:#?}", request);
        let args = request.into_inner();
        info!("Kill querry for {}", args.container_id);
        
        match kill(&args.container_id) {
            Ok(_) => {
                {
                    let mut containers = self.containers.lock().unwrap();
                    if let Some(entry) = containers.get_mut(&args.container_id) {
                        entry.state = ContainerStateCore::Stopped;
                    }
                }
                info!("{} successfully killed", args.container_id);
                Ok(Response::new(KillResponse {}))
            }
            Err(e) => {
                let err = e.to_string();
                error!("Kill: {:#?}", err);
                Err(Status::internal(err))
            },
        }
    }

    async fn delete(&self, request: Request<DeleteRequest>) -> anyhow::Result<Response<DeleteResponse>, Status> {
        debug!("Request: {:#?}", request);
        let args = request.into_inner();
        info!("Delete querry for {}", args.container_id);

        match delete(&args.container_id, args.force) {
            Ok(_) => {
                {
                    let mut containers = self.containers.lock().unwrap();
                    if let Some(entry) = containers.get_mut(&args.container_id) {
                        if entry.state == ContainerStateCore::Stopped {
                            containers.remove_entry(&args.container_id);
                        }
                    }
                }
                info!("{} successfully deleted", args.container_id);
                Ok(Response::new(DeleteResponse { }))
            },
            Err(e) => {
                let err = e.to_string();
                error!("Delete: {:#?}", err);
                Err(Status::internal(err))
            }
        }
    }

}

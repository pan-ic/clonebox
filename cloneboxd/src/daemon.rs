use crate::clonebox_tasks::clonebox_tasks_server::CloneboxTasks;
use crate::clonebox_tasks::{
    CreateRequest, CreateResponse, DeleteRequest, DeleteResponse, ExecRequest, ExecResponse,
    KillRequest, KillResponse, ListRequest, ListResponse, PauseRequest, PauseResponse,
    ResumeRequest, ResumeResponse, StartRequest, StartResponse, State as ProtoState, StateRequest,
    StateResponse, WaitRequest, WaitResponse,
};
use clonebox_core::container::{delete, kill, pause, resume, start, state};
use nix::unistd::setsid;
use std::process::Stdio;
use tokio::{process::Command, sync::oneshot};
use tonic::{Request, Response, Status};
use tracing::{debug, error, info};

use crate::event::get_event_socket_path;

use clonebox_core::state::ContainerState as ContainerStateCore;

use crate::convert::to_status;

use crate::entry::{Cloneboxd, ContainerEntry};

#[tonic::async_trait]
impl CloneboxTasks for Cloneboxd {
    async fn create(
        &self,
        request: Request<CreateRequest>,
    ) -> Result<Response<CreateResponse>, Status> {
        debug!("Request: {:#?}", request);
        let path_to_exec = Cloneboxd::shim_path().map_err(|e| Status::internal(e.to_string()))?;
        let args = request.into_inner();
        info!(
            "Create querry for {}, using {} config_path",
            args.container_id, args.config_path
        );
        let (created_tx, created_rx) = oneshot::channel::<()>();
        let empty_vec = Vec::<oneshot::Sender<i32>>::new();

        let new_entry =
            ContainerEntry::new(ContainerStateCore::Creating, Some(created_tx), empty_vec);

        {
            let mut containers = self.containers.lock().unwrap();
            containers
                .entry(args.container_id.clone())
                .or_insert(new_entry);
        }

        let mut cmd = Command::new(path_to_exec);
        cmd.args([
            &args.container_id,
            &args.config_path,
            &get_event_socket_path(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

        unsafe {
            cmd.pre_exec(|| {
                setsid().map_err(std::io::Error::from)?;
                Ok(())
            })
        };

        cmd.spawn().map_err(|e| Status::internal(e.to_string()))?;

        match created_rx.await {
            Ok(()) => {
                info!("{} successfully created", args.container_id);
                Ok(Response::new(CreateResponse {}))
            }
            Err(_) => {
                let e = "container creation failed".to_string();
                error!("Create: {:#?}", e);
                Err(Status::internal(e))
            }
        }
    }

    async fn start(
        &self,
        request: Request<StartRequest>,
    ) -> Result<Response<StartResponse>, Status> {
        debug!("Request: {:#?}", request);
        let args = request.into_inner();
        info!("Start querry for {}", args.container_id);

        start(&args.container_id)
            .inspect_err(|e| error!("Start: {}", e))
            .map_err(to_status)?;

        {
            let mut containers = self.containers.lock().unwrap();
            if let Some(entry) = containers.get_mut(&args.container_id) {
                entry.state = ContainerStateCore::Running;
            }
        }
        info!("{} successfully started", args.container_id);
        Ok(Response::new(StartResponse {}))
    }

    async fn state(
        &self,
        request: Request<StateRequest>,
    ) -> Result<Response<StateResponse>, Status> {
        debug!("Request: {:#?}", request);
        let args = request.into_inner();
        debug!("State query for {}", args.container_id);

        let s = state(&args.container_id)
            .inspect_err(|e| error!("State: {}", e))
            .map_err(to_status)?;

        info!("{} successfully stated", args.container_id);
        Ok(Response::new(StateResponse {
            state: Some(ProtoState::from(s)),
        }))
    }

    async fn kill(&self, request: Request<KillRequest>) -> Result<Response<KillResponse>, Status> {
        debug!("Request: {:#?}", request);
        let args = request.into_inner();
        info!("Kill querry for {}", args.container_id);

        kill(&args.container_id)
            .inspect_err(|e| error!("Kill: {}", e))
            .map_err(to_status)?;

        {
            let mut containers = self.containers.lock().unwrap();
            if let Some(entry) = containers.get_mut(&args.container_id) {
                entry.state = ContainerStateCore::Stopped;
            }
        }
        info!("{} successfully killed", args.container_id);
        Ok(Response::new(KillResponse {}))
    }

    async fn delete(
        &self,
        request: Request<DeleteRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        debug!("Request: {:#?}", request);
        let args = request.into_inner();
        info!("Delete querry for {}", args.container_id);

        delete(&args.container_id, args.force)
            .inspect_err(|e| error!("Delete: {}", e))
            .map_err(to_status)?;

        {
            let mut containers = self.containers.lock().unwrap();
            if let Some(entry) = containers.get_mut(&args.container_id)
                && entry.state == ContainerStateCore::Stopped
            {
                containers.remove_entry(&args.container_id);
            }
        }
        info!("{} successfully deleted", args.container_id);
        Ok(Response::new(DeleteResponse {}))
    }

    async fn pause(
        &self,
        request: Request<PauseRequest>,
    ) -> Result<Response<PauseResponse>, Status> {
        debug!("Request: {:#?}", request);
        let args = request.into_inner();
        info!("Pause querry for {}", args.container_id);

        pause(&args.container_id)
            .inspect_err(|e| error!("Pause: {}", e))
            .map_err(to_status)?;

        {
            let mut containers = self.containers.lock().unwrap();
            if let Some(entry) = containers.get_mut(&args.container_id) {
                entry.state = ContainerStateCore::Paused;
            }
        }
        info!("{} successfully paused", args.container_id);
        Ok(Response::new(PauseResponse {}))
    }

    async fn resume(
        &self,
        request: Request<ResumeRequest>,
    ) -> Result<Response<ResumeResponse>, Status> {
        debug!("Request: {:#?}", request);
        let args = request.into_inner();
        info!("Resume querry for {}", args.container_id);

        resume(&args.container_id)
            .inspect_err(|e| error!("Resume: {}", e))
            .map_err(to_status)?;

        {
            let mut containers = self.containers.lock().unwrap();
            if let Some(entry) = containers.get_mut(&args.container_id) {
                entry.state = ContainerStateCore::Running;
            }
        }
        info!("{} successfully restarted", args.container_id);
        Ok(Response::new(ResumeResponse {}))
    }

    async fn exec(&self, _request: Request<ExecRequest>) -> Result<Response<ExecResponse>, Status> {
        Err(Status::unimplemented(
            "exec over gRPC is not implemented in v1; use the clonebox CLI directly",
        ))
    }

    async fn wait(&self, request: Request<WaitRequest>) -> Result<Response<WaitResponse>, Status> {
        debug!("Request: {:#?}", request);
        let args = request.into_inner();
        info!("Wainting for {} ..", args.container_id);
        let (exit_tx, exit_rx) = oneshot::channel::<i32>();

        {
            let mut containers = self.containers.lock().unwrap();
            if let Some(entry) = containers.get_mut(&args.container_id) {
                entry.exit_waiter.push(exit_tx);
            } else {
                let err = "not found".to_string();
                error!("Wait: {:#?}", err);
                return Err(Status::internal(err));
            }
        }

        match exit_rx.await {
            Ok(exit) => {
                info!("{} exited: status {}", args.container_id, exit);
                Ok(Response::new(WaitResponse { exit }))
            }
            Err(e) => {
                let err = e.to_string();
                error!("Wait: {:#?}", err);
                Err(Status::internal(err))
            }
        }
    }

    async fn list(&self, request: Request<ListRequest>) -> Result<Response<ListResponse>, Status> {
        debug!("Request: {:#?}", request);
        info!("Containers list querried");
        let ids_list: Vec<String> = {
            let containers = self.containers.lock().unwrap();
            containers.keys().cloned().collect()
        };

        let mut states_list: Vec<ProtoState> = Vec::new();

        for i in ids_list {
            if let Ok(s) = state(&i) {
                states_list.push(ProtoState::from(s));
            }
        }

        info!("Container list successfully returned");
        Ok(Response::new(ListResponse {
            states: states_list,
        }))
    }
}

pub(crate) fn graceful_shutdown(client_sock: &str, event_sock: &str) -> anyhow::Result<()> {
    let _ = std::fs::remove_file(client_sock);
    let _ = std::fs::remove_file(event_sock);
    Ok(())
}

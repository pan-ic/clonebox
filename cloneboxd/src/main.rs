use tonic::{transport::Server, Request, Response, Status};
use clonebox_core::container::{create, delete, exec, kill, pause, resume, start, state};
use clonebox_tasks::clonebox_tasks_server::{CloneboxTasks, CloneboxTasksServer};
use clonebox_tasks::{
    ContainerState,
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
    State,
    StateRequest,
    StateResponse,
    WaitRequest,
    WaitResponse,
};

pub mod clonebox_tasks {
    tonic::include_proto!("clonebox_tasks");
}

#[derive(Debug, Default)]
pub struct Cloneboxd {}

#[tonic::async_trait]
impl CloneboxTasks for Cloneboxd {
    async fn create(&self, request: Request<CreateRequest>) -> Result<Response<CreateResponse>, Status> {
        todo!();
    }

    async fn start(&self, request: Request<StartRequest>) -> Result<Response<StartResponse>, Status> {
        todo!();
    }

    async fn state(&self, request: Request<StateRequest>) -> Result<Response<StateResponse>, Status> {
        todo!();
    }

    async fn kill(&self, request: Request<KillRequest>) -> Result<Response<KillResponse>, Status> {
        todo!();
    }

    async fn delete(&self, request: Request<DeleteRequest>) -> Result<Response<DeleteResponse>, Status> {
        todo!();
    }

    async fn pause(&self, request: Request<PauseRequest>) -> Result<Response<PauseResponse>, Status> {
        todo!();
    }

    async fn resume(&self, request: Request<ResumeRequest>) -> Result<Response<ResumeResponse>, Status> {
        todo!();
    }

    async fn exec(&self, request: Request<ExecRequest>) -> Result<Response<ExecResponse>, Status> {
        todo!();
    }

    async fn wait(&self, request: Request<WaitRequest>) -> Result<Response<WaitResponse>, Status> {
        todo!();
    }

    async fn list(&self, request: Request<ListRequest>) -> Result<Response<ListResponse>, Status> {
        todo!();
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let cloneboxd = Cloneboxd::default();

    Server::builder()
        .add_service(CloneboxTasksServer::new(cloneboxd))
        .serve(addr)
        .await?;

    Ok(())
}

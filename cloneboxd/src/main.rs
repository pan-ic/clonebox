mod event;
mod daemon;
mod recovery;

use anyhow::Context;
use tokio::{
    net::UnixListener,
    process::Command,
    sync::oneshot,
};
use tokio_stream::wrappers::UnixListenerStream;
use tonic::{transport::Server, Request, Response, Status};
use crate::daemon::clonebox_tasks::clonebox_tasks_server::{CloneboxTasks, CloneboxTasksServer};
use std::{
    collections::HashMap,
    env,
    env::current_exe,
    fs::{create_dir_all, Permissions},
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::{Arc, Mutex,},
};

use crate::event::{
    get_app_data_path,
    get_listen_sk,
    event_loop,
};

use clonebox_core::state::ContainerState as ContainerStateCore;

use crate::daemon::Cloneboxd;

use crate::recovery::recover;

pub mod clonebox_tasks {
    tonic::include_proto!("clonebox_tasks");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app_data_path = "/run/clonebox";
    let server_socket = "/run/clonebox/clonebox.sock";
    create_dir_all(app_data_path)?;
    std::fs::set_permissions("/run/clonebox", Permissions::from_mode(0o700))
        .context("failed to chmod clonebox dir")?;
    let cloneboxd = Cloneboxd::new();

    let uds = get_listen_sk(server_socket).await?;
    let uds_stream = UnixListenerStream::new(uds);
    recover(&cloneboxd.containers, app_data_path)?;
    
    //move
    let event_socket_path = get_app_data_path();
    let listener: UnixListener = get_listen_sk(&event_socket_path).await.unwrap();
    event_loop(listener, &cloneboxd.containers).await.unwrap();

    Server::builder()
        .add_service(CloneboxTasksServer::new(cloneboxd))
        .serve_with_incoming(uds_stream)
        .await?;

    Ok(())
}

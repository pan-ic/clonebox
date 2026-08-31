mod convert;
mod daemon;
mod entry;
mod event;
mod recovery;

use crate::clonebox_tasks::clonebox_tasks_server::CloneboxTasksServer;
use anyhow::Context;
use std::{
    fs::{Permissions, create_dir_all},
    os::unix::fs::PermissionsExt,
};
use tokio::{net::UnixListener, signal::ctrl_c};
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;
use tracing_subscriber::filter::EnvFilter;

use crate::event::{event_loop, get_event_socket_path, get_listen_sk};

use crate::entry::Cloneboxd;

use crate::daemon::graceful_shutdown;

use crate::recovery::recover;

pub mod clonebox_tasks {
    tonic::include_proto!("clonebox_tasks");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app_data_path = "/run/clonebox";
    let client_socket = "/run/clonebox/clonebox.sk";
    create_dir_all(app_data_path)?;
    std::fs::set_permissions("/run/clonebox", Permissions::from_mode(0o700))
        .context("failed to chmod clonebox dir")?;
    //launch with: RUST_LOG={level} ./cloneboxd
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    let cloneboxd = Cloneboxd::new();

    let uds = get_listen_sk(client_socket).await?;
    let uds_stream = UnixListenerStream::new(uds);
    recover(&cloneboxd.containers, app_data_path)?;

    //move
    let event_socket_path = get_event_socket_path();
    let listener: UnixListener = get_listen_sk(&event_socket_path).await.unwrap();
    event_loop(listener, &cloneboxd.containers).await.unwrap();

    Server::builder()
        .add_service(CloneboxTasksServer::new(cloneboxd))
        .serve_with_incoming_shutdown(uds_stream, async {
            ctrl_c().await.ok();
        })
        .await?;

    let _ = graceful_shutdown(client_socket, &event_socket_path);

    Ok(())
}

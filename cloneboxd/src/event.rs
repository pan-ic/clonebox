use clonebox_core::{event::Event, state::ContainerState};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::UnixListener,
};

use crate::entry::ContainerEntry;

pub(crate) async fn get_listen_sk(path: &str) -> anyhow::Result<UnixListener> {
    let _ = std::fs::remove_file(path);

    Ok(UnixListener::bind(path).unwrap())
}

fn update_status(
    event: Event,
    containers: &Arc<Mutex<HashMap<String, ContainerEntry>>>,
) -> anyhow::Result<()> {
    let mut h = containers.lock().unwrap();

    if let Some(entry) = h.get_mut(&event.get_id()) {
        entry.state = event.get_state();
        if entry.state != ContainerState::Creating
            && let Some(tx) = entry.created_waiter.take()
        {
            let _ = tx.send(());
        }
        if entry.state == ContainerState::Stopped {
            let code = event.get_exit_code().unwrap_or_default();
            for tx in std::mem::take(&mut entry.exit_waiter) {
                let _ = tx.send(code);
            }
        }
    }

    Ok(())
}

pub(crate) async fn event_loop(
    listener: UnixListener,
    containers: &Arc<Mutex<HashMap<String, ContainerEntry>>>,
) -> anyhow::Result<()> {
    let containers_outer = Arc::clone(containers);

    tokio::spawn(async move {
        loop {
            let containers_inner = Arc::clone(&containers_outer);
            let (conn, _) = listener.accept().await.unwrap();
            let mut lines = BufReader::new(conn).lines();

            tokio::spawn(async move {
                while let Ok(Some(line)) = lines.next_line().await {
                    if let Ok(e) = serde_json::from_str::<Event>(&line) {
                        let _ = update_status(e, &containers_inner);
                    } else {
                        eprintln!("event: event_loop: failed to deserialize");
                    }
                }
            });
        }
    });

    Ok(())
}

pub(crate) fn get_event_socket_path() -> String {
    String::from("/run/clonebox/event.sk")
}

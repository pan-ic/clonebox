use tonic::{transport::Server, Request, Response, Status};
use tonic::IntoRequest;
use clonebox_tasks::clonebox_tasks_client::{CloneboxTasksClient};
use clonebox_tasks::{
    CreateRequest,
    StartRequest,
    DeleteRequest,
    KillRequest,
};

pub mod clonebox_tasks {
    tonic::include_proto!("clonebox_tasks");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = CloneboxTasksClient::connect("/run/clonebox/clonebox.sock").await?;

    let create_req = tonic::Request::new(
        CreateRequest {
            container_id: String::from("test"),
            config_path: String::from("/home/debian/clonebox/clonebox/config"),
        }
    );
    let start_req = tonic::Request::new(
        StartRequest {
            container_id: String::from("test"),
        }
    );
    let delete_req = tonic::Request::new(
        DeleteRequest {
            container_id: String::from("test"),
            force: false,
        }
    );
    let kill_req = tonic::Request::new(
        KillRequest {
            container_id: String::from("test"),
        }
    );

    let create_resp = client.create(create_req).await?;
    let start_resp = client.start(start_req).await?;
    let kill_resp = client.kill(kill_req).await?;
    let delete_resp = client.delete(delete_req).await?;

    println!("CREATE={:?}", create_resp);
    println!("START={:?}", start_resp);
    println!("KILL={:?}", kill_resp);
    println!("DELETE={:?}", delete_resp);

    Ok(())
} 

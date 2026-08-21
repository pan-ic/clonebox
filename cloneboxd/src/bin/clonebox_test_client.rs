use clonebox_tasks::clonebox_tasks_client::{CloneboxTasksClient};
use clonebox_tasks::StateRequest;

pub mod clonebox_tasks {
    tonic::include_proto!("clonebox_tasks");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = CloneboxTasksClient::connect("http://[::1]:50051").await?;

    let request = tonic::Request::new(StateRequest {
        container_id: "test".into(),
    });

    let response = client.state(request).await?;

    println!("RESPONSE={:?}", response);

    Ok(())
} 

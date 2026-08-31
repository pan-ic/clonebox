use clap::{Parser, Subcommand};
use clonebox_core::container::exec;
use clonebox_tasks::clonebox_tasks_client::CloneboxTasksClient;
use clonebox_tasks::{
    CreateRequest, DeleteRequest, KillRequest, ListRequest, PauseRequest, ResumeRequest,
    StartRequest, StateRequest, WaitRequest,
};

pub mod clonebox_tasks {
    tonic::include_proto!("clonebox_tasks");
}

#[derive(Debug, Parser)]
#[command(version, author, about)]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(arg_required_else_help = true, about = "Creates a container")]
    Create {
        #[arg(required = true, help = "container name")]
        container_id: String,
        #[arg(required = true, help = "path to the bundle/config directory")]
        config: String,
    },
    #[command(
        arg_required_else_help = true,
        about = "Starts an already created container"
    )]
    Start {
        #[arg(required = true, help = "container name")]
        container_id: String,
    },
    #[command(
        arg_required_else_help = true,
        about = "Query and print an existing container state"
    )]
    State {
        #[arg(required = true, help = "container name")]
        container_id: String,
    },
    #[command(arg_required_else_help = true, about = "Stops a running container")]
    Kill {
        #[arg(required = true, help = "container name")]
        container_id: String,
    },
    #[command(arg_required_else_help = true, about = "Deletes a stopped container")]
    Delete {
        #[arg(required = true, help = "container name")]
        container_id: String,
        #[arg(short, long, help = "force deletes, optional")]
        force: bool,
    },
    #[command(arg_required_else_help = true, about = "Pauses a running container")]
    Pause {
        #[arg(required = true, help = "container name")]
        container_id: String,
    },
    #[command(arg_required_else_help = true, about = "Resume a paused container")]
    Resume {
        #[arg(required = true, help = "container name")]
        container_id: String,
    },
    #[command(
        arg_required_else_help = true,
        about = "Execute a command inside a running container (current version bypasses daemon, see readme)"
    )]
    Exec {
        #[arg(required = true, help = "container name")]
        container_id: String,
        //#[arg(trailing_var_arg = true)] //that version doesn't need the -- separtor ans also
        //bundle everything after
        #[arg(last = true, help = "command to run")]
        cmd: Vec<String>,
    },
    #[command(about = "Querry daemon about all existing containers")]
    List,
    #[command(
        arg_required_else_help = true,
        about = "Hold the current command until container exits"
    )]
    Wait {
        #[arg(required = true, help = "container name")]
        container_id: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let path = "unix:///run/clonebox/clonebox.sock";
    let mut client = CloneboxTasksClient::connect(path).await?;

    match cli.cmd {
        Commands::Create {
            container_id,
            config,
        } => {
            let create_req = tonic::Request::new(CreateRequest {
                container_id,
                config_path: config,
            });
            println!("{:?}", client.create(create_req).await?);
        }
        Commands::Start { container_id } => {
            let start_req = tonic::Request::new(StartRequest { container_id });
            println!("{:?}", client.start(start_req).await?);
        }
        Commands::State { container_id } => {
            let state_req = tonic::Request::new(StateRequest { container_id });
            println!("{:?}", client.state(state_req).await?);
        }
        Commands::Kill { container_id } => {
            let kill_req = tonic::Request::new(KillRequest { container_id });
            println!("{:?}", client.kill(kill_req).await?);
        }
        Commands::Delete {
            container_id,
            force,
        } => {
            let delete_req = tonic::Request::new(DeleteRequest {
                container_id,
                force,
            });
            println!("{:?}", client.delete(delete_req).await?);
        }
        Commands::Pause { container_id } => {
            let pause_req = tonic::Request::new(PauseRequest { container_id });
            println!("{:?}", client.pause(pause_req).await?);
        }
        Commands::Resume { container_id } => {
            let resume_req = tonic::Request::new(ResumeRequest { container_id });
            println!("{:?}", client.resume(resume_req).await?);
        }
        Commands::Exec { container_id, cmd } => {
            exec(&container_id, cmd)?;
        }
        Commands::List => {
            let list_req = tonic::Request::new(ListRequest {});
            println!("{:?}", client.list(list_req).await?);
        }
        Commands::Wait { container_id } => {
            let wait_req = tonic::Request::new(WaitRequest { container_id });
            println!("{:?}", client.wait(wait_req).await?);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parse() {
        let cli = Cli::try_parse_from(["clonebox", "create", "test", "tests_config"]).unwrap();

        match cli.cmd {
            Commands::Create {
                container_id,
                config,
            } => {
                assert_eq!(container_id, "test");
                assert_eq!(config, "tests_config");
            }
            _ => {
                panic!("KO")
            }
        };
    }
}

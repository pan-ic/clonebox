use clonebox_core::container::{create, delete, exec, kill, pause, resume, start, state};
use clap::{Parser, Subcommand};
use std::fs::create_dir_all;

#[derive(Debug, Parser)]
#[command(version, author, about)]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

//TODO: needs cleaner help
#[derive(Debug, Subcommand)]
enum Commands {
    #[command(arg_required_else_help = true)]
    Create {
        #[arg(required = true)]
        container_id: String,
        #[arg(required = true)]
        config: String,
    },
    #[command(arg_required_else_help = true)]
    Start {
        #[arg(required = true)]
        container_id: String,
    },
    #[command(arg_required_else_help = true)]
    State {
        #[arg(required = true)]
        container_id: String,
    },
    #[command(arg_required_else_help = true)]
    Kill {
        #[arg(required = true)]
        container_id: String,
    },
    #[command(arg_required_else_help = true)]
    Delete {
        #[arg(required = true)]
        container_id: String,
        #[arg(short, long)]
        force: bool,
    },
    #[command(arg_required_else_help = true)]
    Pause {
        #[arg(required = true)]
        container_id: String,
    },
    #[command(arg_required_else_help = true)]
    Resume {
        #[arg(required = true)]
        container_id: String,
    },
    #[command(arg_required_else_help = true)]
    Exec {
        #[arg(required = true)]
        container_id: String,
        //#[arg(trailing_var_arg = true)] //that version doesn't need the -- separtor ans also
        //bundle everything after
        #[arg(last = true)]
        cmd: Vec<String>,
    },
}

fn main() -> anyhow::Result<()> {
    create_dir_all("/run/clonebox")?;
    let cli = Cli::parse();

    match cli.cmd {
        Commands::Create {
            container_id,
            config,
        } => {
            create(&container_id, &config)?;
        }
        Commands::Start { container_id } => {
            start(&container_id)?;
        }
        Commands::State { container_id } => {
            let state = state(&container_id)?;
            print!("{}", state);
        }
        Commands::Kill { container_id } => {
            kill(&container_id)?;
        }
        Commands::Delete {
            container_id,
            force,
        } => {
            delete(&container_id, force)?;
        }
        Commands::Pause { container_id } => {
            pause(&container_id)?;
        }
        Commands::Resume { container_id } => {
            resume(&container_id)?;
        }
        Commands::Exec { container_id, cmd } => {
            exec(&container_id, cmd)?;
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

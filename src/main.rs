mod cgroup;
mod container;
mod namespace;
mod network;
mod clone3;

use crate::container::create_child_process;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(version, author, about)]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(arg_required_else_help = true)]
    Run {
        #[arg(long, default_value = "/run/clonebox")]
        config: Option<String>,
        #[arg(long, required = true)]
        name: String,
        //temporary solution adopted for manual tests, will be replaced soon by config parsing
        #[arg(long, required = true)]
        cmd: String,
    },
    /*
    #[command(arg_required_else_help = true)]
    Start {}
    */
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
        Commands::Run { name, cmd, .. } => {
            println!("Container {} starts", name);
            create_child_process(&name, &cmd)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parse() {
        let cli =
            Cli::try_parse_from(["clonebox", "run", "--name", "test", "--cmd", "echo OK"]).unwrap();

        match cli.cmd {
            Commands::Run { config, name, cmd } => {
                assert_eq!(config, Some(String::from("/run/clonebox")));
                assert_eq!(name, "test");
                assert_eq!(cmd, "echo OK");
            }
        };
    }
}

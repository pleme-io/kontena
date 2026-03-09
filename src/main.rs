mod colima;
mod podman;
mod util;

use clap::{Parser, Subcommand};
use tracing::error;

#[derive(Parser)]
#[command(name = "kontena", about = "Container runtime management daemon")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage podman machine lifecycle
    Podman {
        #[command(subcommand)]
        action: PodmanAction,
    },
    /// Manage colima VM lifecycle
    Colima {
        #[command(subcommand)]
        action: ColimaAction,
    },
}

#[derive(Subcommand)]
enum PodmanAction {
    /// Initialize the podman machine if it does not exist
    Init,
    /// Start the podman machine and monitor its state
    Start,
}

#[derive(Subcommand)]
enum ColimaAction {
    /// Start colima in foreground mode (exec replaces process)
    Start,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Podman { action } => match action {
            PodmanAction::Init => podman::init::run(),
            PodmanAction::Start => podman::start::run(),
        },
        Commands::Colima { action } => match action {
            ColimaAction::Start => colima::start::run(),
        },
    };

    if let Err(err) = result {
        error!("{err:#}");
        std::process::exit(1);
    }
}

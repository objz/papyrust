use anyhow::Result;
use clap::Parser;
use log::{error, info};
use std::process;

mod cache;
mod config;
mod ipc;
mod renderer;
mod video;
mod wayland;

use config::Config;
use wayland::WaylandClient;

#[derive(Parser)]
#[command(name = "papyrust-daemon")]
#[command(about = "Wayland wallpaper daemon with image, GIF, and video support")]
struct Args {
    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,

    /// Socket path (default: $XDG_RUNTIME_DIR/papyrust.socket)
    #[arg(short, long)]
    socket: Option<String>,

    /// Default scaling mode
    #[arg(long, default_value = "fill")]
    mode: String,

    /// Disable transitions
    #[arg(long)]
    no_transitions: bool,

    /// Transition duration in milliseconds
    #[arg(long, default_value = "300")]
    transition_duration: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    if args.debug {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .init();
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Info)
            .init();
    }

    info!("Starting Papyrust daemon");

    // Create configuration
    let config = Config::new(args)?;

    // Initialize Wayland client
    let mut wayland_client = match WaylandClient::new(&config).await {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to initialize Wayland client: {}", e);
            process::exit(1);
        }
    };

    // Start IPC server
    let ipc_handle = ipc::start_server(&config, wayland_client.get_sender()).await?;

    info!("Daemon started successfully");

    // Run the main event loop
    tokio::select! {
        result = wayland_client.run() => {
            if let Err(e) = result {
                error!("Wayland client error: {}", e);
            }
        }
        result = ipc_handle => {
            if let Err(e) = result {
                error!("IPC server error: {}", e);
            }
        }
    }

    info!("Daemon shutting down");
    Ok(())
}

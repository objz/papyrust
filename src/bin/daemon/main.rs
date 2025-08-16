use anyhow::Result;
use clap::{Parser, ValueEnum};
use std::{process, sync::mpsc, thread};

// NEW: tracing imports
use tracing::info;
use tracing_log::LogTracer;
use tracing_subscriber::{fmt, EnvFilter};

mod ipc;
mod media;
mod wayland;
mod utils;
mod gl_bindings {
    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}

#[derive(ValueEnum, Clone, Debug)]
enum Layer {
    Bottom,
    Top,
    Overlay,
    Background,
}

impl std::fmt::Display for Layer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Layer::Bottom => "bottom",
            Layer::Top => "top",
            Layer::Overlay => "overlay",
            Layer::Background => "background",
        };
        write!(f, "{}", s)
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "papyrust-daemon",
    version = "0.1.0",
    about = "A Wayland wallpaper daemon with OpenGL ES shader support for images, videos, and shaders"
)]
struct Args {
    #[arg(short = 'F', long)]
    fork: bool,

    #[arg(short, long, default_value = "30")]
    fps: u16,

    #[arg(short, long)]
    layer: Option<Layer>,

    #[arg(short = 'M', long)]
    fifo: Option<String>,

    #[arg(long, alias = "no-audio")]
    mute: bool,
}

fn main() -> Result<()> {
    // NEW: structured logging
    let _ = LogTracer::init(); // forward `log` records into `tracing`
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("papyrust=info,wayland_client=warn"));
    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .compact()
        .init();

    let args = Args::parse();

    if args.fork {
        unsafe {
            let pid = libc::fork();
            if pid > 0 {
                process::exit(0);
            }
            if pid == 0 {
                libc::close(0);
                libc::close(1);
                libc::close(2);
            }
        }
    }

    let (tx, rx) = mpsc::channel();

    let ipc_tx = tx.clone();
    thread::spawn(move || {
        if let Err(e) = ipc::start_server(ipc_tx) {
            tracing::error!("IPC server error: {e}");
        }
    });

    let init_media = media::MediaType::Shader("default".to_string());

    info!("Starting Papyrust daemon");

    wayland::init(
        init_media,
        args.fps,
        args.layer.as_ref().map(|l| l.to_string()).as_deref(),
        args.fifo.as_deref(),
        rx,
        args.mute,
    )?;

    Ok(())
}

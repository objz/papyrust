use anyhow::Result;
use clap::Parser;
use log::info;
use std::process;
use std::sync::mpsc;
use std::thread;

mod ipc;
mod media;
mod paper;
mod utils;
mod gl_bindings {
    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}

#[derive(Parser, Debug)]
#[command(
    name = "papyrust-daemon",
    version = "0.1.0",
    about = "A Wayland wallpaper daemon with OpenGL ES shader support for images, videos, and shaders"
)]
struct Args {
    /// Fork papyrust-daemon
    #[arg(short = 'F', long)]
    fork: bool,

    /// Set the FPS to render at
    #[arg(short, long, default_value = "0")]
    fps: u16,

    /// Specify layer to run on
    #[arg(short, long)]
    layer: Option<String>,

    /// The width to render at
    #[arg(short = 'W', long, default_value = "0")]
    width: u16,

    /// The height to render at
    #[arg(short = 'H', long, default_value = "0")]
    height: u16,

    /// The path to mpd's fifo file
    #[arg(short = 'M', long)]
    fifo: Option<String>,
}

fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();

    if args.fork {
        unsafe {
            let pid = libc::fork();
            if pid > 0 {
                process::exit(0);
            }
            if pid == 0 {
                libc::close(0); // stdin
                libc::close(1); // stdout
                libc::close(2); // stderr
            }
        }
    }

    let (tx, rx) = mpsc::channel();

    let ipc_tx = tx.clone();
    thread::spawn(move || {
        if let Err(e) = ipc::start_server(ipc_tx) {
            eprintln!("IPC server error: {}", e);
        }
    });

    let init_media = media::MediaType::Shader("default".to_string());

    info!("Starting Papyrust daemon");

    let monitor = "";

    paper::init(
        monitor,
        init_media,
        args.fps,
        args.layer.as_deref(),
        args.width,
        args.height,
        args.fifo.as_deref(),
        rx,
    )?;

    Ok(())
}

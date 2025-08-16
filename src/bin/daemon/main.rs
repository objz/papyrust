use clap::ValueEnum;
use anyhow::Result;
use clap::Parser;
use log::info;
use std::process;
use std::sync::mpsc;
use std::thread;

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

    #[arg(short, long)]
    fps: Option<u16>,

    #[arg(short, long)]
    layer: Option<Layer>,

    #[arg(short = 'M', long)]
    fifo: Option<String>,

    #[arg(long, alias = "no-audio")]
    mute: bool,
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
            eprintln!("IPC server error: {}", e);
        }
    });

    let init_media = media::MediaType::Shader("default".to_string());

    info!("Starting Papyrust daemon");

    wayland::init(
        init_media,
        args.fps.unwrap_or(0), 
        args.layer.as_ref().map(|l| l.to_string()).as_deref(),
        args.fifo.as_deref(),
        rx,
        args.mute,
    )?;

    Ok(())
}

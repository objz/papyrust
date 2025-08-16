use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Parser)]
#[command(name = "papyrust")]
#[command(about = "A small cli for papyrust-daemon")]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Image {
        path: String,
        #[arg(long)]
        shader: Option<String>,
        #[arg(long)]
        monitor: Option<String>,
    },
    Video {
        path: String,
        #[arg(long)]
        shader: Option<String>,
        #[arg(long)]
        monitor: Option<String>,
        #[arg(long)]
        mute: bool,
    },
    Shader {
        path: String,
        #[arg(long)]
        monitor: Option<String>,
    },
}

fn main() -> Result<()> {
    let _ = fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("off")),
        )
        .with_target(true)
        .compact()
        .try_init();
    let args = Args::parse();

    let mut stream = UnixStream::connect("/tmp/papyrust-daemon.sock")?;

    let command = match args.command {
        Commands::Image {
            path,
            shader,
            monitor,
        } => json!({
            "SetImage": {
                "path": path,
                "shader": shader,
                "monitor": monitor
            }
        }),
        Commands::Video {
            path,
            shader,
            monitor,
            mute,
        } => json!({
            "SetVideo": {
                "path": path,
                "shader": shader,
                "monitor": monitor,
                "mute": mute
            }
        }),
        Commands::Shader { path, monitor } => json!({
            "SetShader": {
                "path": path,
                "monitor": monitor
            }
        }),
    };

    writeln!(stream, "{}", command)?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response)?;

    println!("{}", response.trim());
    Ok(())
}

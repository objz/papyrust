use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

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
    },
    Shader {
        path: String,
        #[arg(long)]
        monitor: Option<String>,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();

    let mut stream = UnixStream::connect("/tmp/papyrust-daemon.sock")?;

    let command = match args.command {
        Commands::Image {
            path,
            shader,
            monitor,
        } => {
            serde_json::json!({
                "SetImage": {
                    "path": path,
                    "shader": shader,
                    "monitor": monitor
                }
            })
        }
        Commands::Video {
            path,
            shader,
            monitor,
        } => {
            serde_json::json!({
                "SetVideo": {
                    "path": path,
                    "shader": shader,
                    "monitor": monitor
                }
            })
        }
        Commands::Shader { path, monitor } => {
            serde_json::json!({
                "SetShader": {
                    "path": path,
                    "monitor": monitor
                }
            })
        }
    };

    writeln!(stream, "{}", command)?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response)?;

    println!("{}", response.trim());

    Ok(())
}

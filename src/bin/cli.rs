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
    /// Set an image as wallpaper
    Image {
        /// Path to image file
        path: String,
        /// Optional shader file to apply effects
        #[arg(long)]
        shader: Option<String>,
    },
    /// Set a video as wallpaper
    Video {
        /// Path to video file
        path: String,
        /// Optional shader file to apply effects
        #[arg(long)]
        shader: Option<String>,
    },
    /// Set a pure shader as wallpaper
    Shader {
        /// Path to shader file
        path: String,
    },
    /// Get current status
    Status,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let mut stream = UnixStream::connect("/tmp/papyrust-daemon.sock")?;

    let command = match args.command {
        Commands::Image { path, shader } => {
            serde_json::json!({
                "SetImage": {
                    "path": path,
                    "shader": shader
                }
            })
        }
        Commands::Video { path, shader } => {
            serde_json::json!({
                "SetVideo": {
                    "path": path,
                    "shader": shader
                }
            })
        }
        Commands::Shader { path } => {
            serde_json::json!({
                "SetShader": {
                    "path": path
                }
            })
        }
        Commands::Status => {
            serde_json::json!("GetStatus")
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

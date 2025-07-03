use anyhow::Result;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::config::{Config, ScalingMode};

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "cmd")]
pub enum Command {
    #[serde(rename = "set_image")]
    SetImage {
        path: String,
        output: String,
        #[serde(default)]
        mode: Option<String>,
    },
    #[serde(rename = "set_video")]
    SetVideo {
        path: String,
        output: String,
        #[serde(default = "default_true")]
        r#loop: bool,
        #[serde(default = "default_false")]
        audio: bool,
    },
    #[serde(rename = "set_gif")]
    SetGif {
        path: String,
        output: String,
        #[serde(default)]
        mode: Option<String>,
    },
    #[serde(rename = "query")]
    Query { output: String },
    #[serde(rename = "stop")]
    Stop { output: String },
    #[serde(rename = "exit")]
    Exit,
}

#[derive(Debug, Clone, Serialize)]
pub struct Response {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}

pub async fn start_server(
    config: &Config,
    wayland_sender: mpsc::Sender<WaylandCommand>,
) -> Result<JoinHandle<Result<()>>> {
    let socket_path = config.socket_path.clone();

    // Remove existing socket file if it exists
    if socket_path.exists() {
        tokio::fs::remove_file(&socket_path).await?;
    }

    let listener = UnixListener::bind(&socket_path)?;
    info!("IPC server listening on {}", socket_path.display());

    let handle = tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let sender = wayland_sender.clone();
                    tokio::spawn(handle_client(stream, sender));
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    });

    Ok(handle)
}

async fn handle_client(
    stream: UnixStream,
    wayland_sender: mpsc::Sender<WaylandCommand>,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break, // EOF
            Ok(_) => {
                let response = match serde_json::from_str::<Command>(&line.trim()) {
                    Ok(command) => {
                        debug!("Received command: {:?}", command);
                        handle_command(command, &wayland_sender).await
                    }
                    Err(e) => Response {
                        success: false,
                        message: format!("Invalid JSON: {}", e),
                        data: None,
                    },
                };

                let response_json = serde_json::to_string(&response)?;
                writer.write_all(response_json.as_bytes()).await?;
                writer.write_all(b"\n").await?;
            }
            Err(e) => {
                error!("Failed to read from client: {}", e);
                break;
            }
        }
    }

    Ok(())
}

async fn handle_command(
    command: Command,
    wayland_sender: &mpsc::Sender<WaylandCommand>,
) -> Response {
    use Command::*;

    match command {
        SetImage { path, output, mode } => {
            let scaling_mode = parse_scaling_mode(mode.as_deref());
            let wayland_cmd = WaylandCommand::SetImage {
                path,
                output,
                mode: scaling_mode,
            };

            send_wayland_command(wayland_sender, wayland_cmd).await
        }
        SetVideo {
            path,
            output,
            r#loop,
            audio,
        } => {
            let wayland_cmd = WaylandCommand::SetVideo {
                path,
                output,
                r#loop,
                audio,
            };

            send_wayland_command(wayland_sender, wayland_cmd).await
        }
        SetGif { path, output, mode } => {
            let scaling_mode = parse_scaling_mode(mode.as_deref());
            let wayland_cmd = WaylandCommand::SetGif {
                path,
                output,
                mode: scaling_mode,
            };

            send_wayland_command(wayland_sender, wayland_cmd).await
        }
        Query { output } => {
            let wayland_cmd = WaylandCommand::Query { output };
            send_wayland_command(wayland_sender, wayland_cmd).await
        }
        Stop { output } => {
            let wayland_cmd = WaylandCommand::Stop { output };
            send_wayland_command(wayland_sender, wayland_cmd).await
        }
        Exit => {
            info!("Received exit command");
            std::process::exit(0);
        }
    }
}

async fn send_wayland_command(
    sender: &mpsc::Sender<WaylandCommand>,
    command: WaylandCommand,
) -> Response {
    match sender.send(command).await {
        Ok(_) => Response {
            success: true,
            message: "Command sent successfully".to_string(),
            data: None,
        },
        Err(e) => Response {
            success: false,
            message: format!("Failed to send command: {}", e),
            data: None,
        },
    }
}

fn parse_scaling_mode(mode: Option<&str>) -> ScalingMode {
    match mode {
        Some("fill") => ScalingMode::Fill,
        Some("fit") => ScalingMode::Fit,
        Some("center") => ScalingMode::Center,
        Some("tile") => ScalingMode::Tile,
        _ => ScalingMode::Fill,
    }
}

#[derive(Debug, Clone)]
pub enum WaylandCommand {
    SetImage {
        path: String,
        output: String,
        mode: ScalingMode,
    },
    SetVideo {
        path: String,
        output: String,
        r#loop: bool,
        audio: bool,
    },
    SetGif {
        path: String,
        output: String,
        mode: ScalingMode,
    },
    Query {
        output: String,
    },
    Stop {
        output: String,
    },
}

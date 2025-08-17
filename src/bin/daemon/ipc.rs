use crate::media::MediaType;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::mpsc::Sender;
use std::thread;

#[derive(Debug, Serialize, Deserialize)]
pub enum IpcCommand {
    SetImage {
        path: String,
        shader: Option<String>,
        monitors: Option<Vec<String>>,
    },
    SetVideo {
        path: String,
        shader: Option<String>,
        monitors: Option<Vec<String>>,
        #[serde(default)]
        mute: bool,
    },
    SetShader {
        path: String,
        monitors: Option<Vec<String>>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IpcResponse {
    Success,
    Error { message: String },
    Status { current_media: String },
}

#[derive(Debug, Clone)]
pub struct MediaChange {
    pub media_type: MediaType,
    pub monitors: Option<Vec<String>>,
    pub mute: bool,
}

pub fn start_server(tx: Sender<MediaChange>) -> Result<()> {
    let socket_path = "/tmp/papyrust-daemon.sock";
    let _ = std::fs::remove_file(socket_path);

    let listener =
        UnixListener::bind(socket_path).map_err(|e| anyhow!("Failed to bind IPC socket: {}", e))?;

    tracing::info!(
        event = "ipc_listen",
        path = socket_path,
        "IPC server listening"
    );
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let tx_clone = tx.clone();
                thread::spawn(move || {
                    if let Err(e) = handle_client(stream, tx_clone) {
                        tracing::warn!(event = "ipc_client_error", error = %e, "Client handling error");
                    }
                });
            }
            Err(e) => {
                tracing::warn!(event = "ipc_accept_error", error = %e, "IPC accept failed");
            }
        }
    }

    Ok(())
}

fn handle_client(stream: UnixStream, tx: Sender<MediaChange>) -> Result<()> {
    let peer = stream.peer_addr().ok();
    tracing::debug!(event = "ipc_client_begin", ?peer, "Client connected");

    let mut reader = BufReader::new(&stream);
    let mut writer = stream.try_clone()?;
    let mut line = String::new();

    while reader.read_line(&mut line)? > 0 {
        let trimmed = line.trim();
        let command: IpcCommand =
            serde_json::from_str(trimmed).map_err(|e| anyhow!("Invalid JSON command: {}", e))?;

        match &command {
            IpcCommand::SetImage { monitors, path, .. } => {
                let target_desc = match monitors {
                    None => "all monitors".to_string(),
                    Some(mons) => format!("monitors: {}", mons.join(", ")),
                };
                tracing::info!(event = "ipc_command", cmd = "SetImage", target = %target_desc, path = %path, "Applying image");
            }
            IpcCommand::SetVideo {
                monitors,
                path,
                mute,
                ..
            } => {
                let target_desc = match monitors {
                    None => "all monitors".to_string(),
                    Some(mons) => format!("monitors: {}", mons.join(", ")),
                };
                tracing::info!(event = "ipc_command", cmd = "SetVideo", target = %target_desc, path = %path, mute = *mute, "Applying video");
            }
            IpcCommand::SetShader { monitors, path } => {
                let target_desc = match monitors {
                    None => "all monitors".to_string(),
                    Some(mons) => format!("monitors: {}", mons.join(", ")),
                };
                tracing::info!(event = "ipc_command", cmd = "SetShader", target = %target_desc, path = %path, "Applying shader");
            }
        }

        let response = match command {
            IpcCommand::SetImage {
                path,
                shader,
                monitors,
            } => {
                let media_change = MediaChange {
                    media_type: MediaType::Image { path, shader },
                    monitors,
                    mute: false,
                };
                match tx.send(media_change) {
                    Ok(_) => IpcResponse::Success,
                    Err(e) => IpcResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            IpcCommand::SetVideo {
                path,
                shader,
                monitors,
                mute,
            } => {
                let media_change = MediaChange {
                    media_type: MediaType::Video { path, shader },
                    monitors,
                    mute,
                };
                match tx.send(media_change) {
                    Ok(_) => IpcResponse::Success,
                    Err(e) => IpcResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            IpcCommand::SetShader { path, monitors } => {
                let media_change = MediaChange {
                    media_type: MediaType::Shader(path),
                    monitors,
                    mute: false,
                };
                match tx.send(media_change) {
                    Ok(_) => IpcResponse::Success,
                    Err(e) => IpcResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
        };

        let response_json = serde_json::to_string(&response)?;
        writeln!(writer, "{}", response_json)?;
        writer.flush()?;

        tracing::debug!(event = "ipc_reply", response = %response_json, "Sent reply to client");
        line.clear();
    }

    tracing::debug!(event = "ipc_client_end", "Client disconnected");
    Ok(())
}

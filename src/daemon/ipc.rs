use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::mpsc::Sender;
use std::thread;

use crate::media::MediaType;

#[derive(Debug, Serialize, Deserialize)]
pub enum IpcCommand {
    SetImage {
        path: String,
        shader: Option<String>,
        monitor: Option<String>,
    },
    SetVideo {
        path: String,
        shader: Option<String>,
        monitor: Option<String>,
    },
    SetShader {
        path: String,
        monitor: Option<String>,
    },
    GetStatus,
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
    pub monitor: Option<String>,
}

pub fn start_server(tx: Sender<MediaChange>) -> Result<()> {
    let socket_path = "/tmp/papyrust-daemon.sock";

    let _ = std::fs::remove_file(socket_path);

    let listener =
        UnixListener::bind(socket_path).map_err(|e| anyhow!("Failed to bind IPC socket: {}", e))?;

    println!("IPC server listening on {}", socket_path);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let tx_clone = tx.clone();
                thread::spawn(move || {
                    if let Err(e) = handle_client(stream, tx_clone) {
                        eprintln!("Client error: {}", e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Connection failed: {}", e);
            }
        }
    }

    Ok(())
}

fn handle_client(stream: UnixStream, tx: Sender<MediaChange>) -> Result<()> {
    let mut reader = BufReader::new(&stream);
    let mut writer = stream.try_clone()?;
    let mut line = String::new();

    while reader.read_line(&mut line)? > 0 {
        let command: IpcCommand = serde_json::from_str(&line.trim())
            .map_err(|e| anyhow!("Invalid JSON command: {}", e))?;

        let response = match command {
            IpcCommand::SetImage {
                path,
                shader,
                monitor,
            } => {
                let media_change = MediaChange {
                    media_type: MediaType::Image { path, shader },
                    monitor,
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
                monitor,
            } => {
                let media_change = MediaChange {
                    media_type: MediaType::Video { path, shader },
                    monitor,
                };
                match tx.send(media_change) {
                    Ok(_) => IpcResponse::Success,
                    Err(e) => IpcResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            IpcCommand::SetShader { path, monitor } => {
                let media_change = MediaChange {
                    media_type: MediaType::Shader(path),
                    monitor,
                };
                match tx.send(media_change) {
                    Ok(_) => IpcResponse::Success,
                    Err(e) => IpcResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            IpcCommand::GetStatus => IpcResponse::Status {
                current_media: "Unknown".to_string(),
            },
        };

        let response_json = serde_json::to_string(&response)?;
        writeln!(writer, "{}", response_json)?;
        writer.flush()?;

        line.clear();
    }

    Ok(())
}

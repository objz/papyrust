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
    },
    SetVideo {
        path: String,
        shader: Option<String>,
    },
    SetShader {
        path: String,
    },
    GetStatus,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IpcResponse {
    Success,
    Error { message: String },
    Status { current_media: String },
}

pub fn start_server(tx: Sender<MediaType>) -> Result<()> {
    let socket_path = "/tmp/papyrust-daemon.sock";

    // Remove existing
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

fn handle_client(stream: UnixStream, tx: Sender<MediaType>) -> Result<()> {
    let mut reader = BufReader::new(&stream);
    let mut writer = stream.try_clone()?;
    let mut line = String::new();

    while reader.read_line(&mut line)? > 0 {
        let command: IpcCommand = serde_json::from_str(&line.trim())
            .map_err(|e| anyhow!("Invalid JSON command: {}", e))?;

        let response = match command {
            IpcCommand::SetImage { path, shader } => {
                let media_type = MediaType::Image { path, shader };
                match tx.send(media_type) {
                    Ok(_) => IpcResponse::Success,
                    Err(e) => IpcResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            IpcCommand::SetVideo { path, shader } => {
                let media_type = MediaType::Video { path, shader };
                match tx.send(media_type) {
                    Ok(_) => IpcResponse::Success,
                    Err(e) => IpcResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            IpcCommand::SetShader { path } => {
                let media_type = MediaType::Shader(path);
                match tx.send(media_type) {
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

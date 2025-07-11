use anyhow::Result;
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

const SOCKET_PATH: &str = "/tmp/papyrust-daemon.sock";

pub fn set_image(monitor: String, path: String, shader: Option<String>) -> Result<()> {
    let cmd = json!({
        "SetImage": {
            "path": path,
            "shader": shader,
            "monitor": monitor
        }
    });
    send_command(cmd)
}

pub fn set_video(monitor: String, path: String, shader: Option<String>) -> Result<()> {
    let cmd = json!({
        "SetVideo": {
            "path": path,
            "shader": shader,
            "monitor": monitor
        }
    });
    send_command(cmd)
}

pub fn set_shader(monitor: String, path: String) -> Result<()> {
    let cmd = json!({
        "SetShader": {
            "path": path,
            "monitor": monitor
        }
    });
    send_command(cmd)
}

fn send_command(cmd: serde_json::Value) -> Result<()> {
    let mut stream = UnixStream::connect(SOCKET_PATH)?;
    writeln!(stream, "{}", cmd)?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response)?;

    println!("{}", response.trim());
    Ok(())
}

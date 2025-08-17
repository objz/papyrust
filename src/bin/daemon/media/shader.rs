use anyhow::{anyhow, Result};
use std::fs::File;
use std::io::Read;

pub fn load_shader(path: &str) -> Result<String> {
    let mut file =
        File::open(path).map_err(|e| anyhow!("Failed to open shader file {}: {}", path, e))?;
    let mut source = String::new();
    file.read_to_string(&mut source)
        .map_err(|e| anyhow!("Failed to read shader file {}: {}", path, e))?;
    Ok(source)
}

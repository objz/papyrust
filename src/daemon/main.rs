use std::fs;
use std::path::Path;
use std::process;
use clap::Parser;

/// Papyrust daemon for applying wallpapers from shader files
#[derive(Parser, Debug)]
#[command(name = "papyrust-daemon")]
#[command(about = "A daemon for applying wallpapers from shader files")]
#[command(version = "0.1.0")]
struct Args {
    /// Output monitor name
    #[arg(index = 1, help = "Output monitor name (e.g., DP-1, HDMI-A-1)")]
    output: String,

    /// Path to shader file
    #[arg(index = 2, help = "Path to shader file", value_parser = validate_shader_path)]
    shader: String,
}

fn validate_shader_path(path: &str) -> Result<String, String> {
    // Expand tilde and environment variables
    let expanded_path = shellexpand::full(path)
        .map_err(|e| format!("Failed to expand path '{}': {}", path, e))?;
    
    let path_buf = Path::new(expanded_path.as_ref());
    
    // Check if file exists
    if !path_buf.exists() {
        return Err(format!("Shader file does not exist: {}", expanded_path));
    }
    
    // Check if it's a file (not a directory)
    if !path_buf.is_file() {
        return Err(format!("Path is not a file: {}", expanded_path));
    }
    
    // Check if file is readable
    match fs::File::open(path_buf) {
        Ok(_) => {},
        Err(e) => return Err(format!("Cannot read shader file '{}': {}", expanded_path, e)),
    }
    
    // Validate file extension (common shader extensions)
    let extension = path_buf.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");
    
    match extension.to_lowercase().as_str() {
        "frag" | "vert" | "glsl" | "shader" => {},
        _ => return Err(format!("Unexpected file extension '{}'. Expected shader file extensions: .frag, .vert, .glsl, .shader", extension)),
    }
    
    Ok(expanded_path.into_owned())
}

fn validate_output_name(output: &str) -> Result<(), String> {
    // Basic validation for output names
    if output.is_empty() {
        return Err("Output name cannot be empty".to_string());
    }
    
    if output.contains(' ') {
        return Err("Output name cannot contain spaces".to_string());
    }
    
    // Check if it matches common output name patterns
    if !output.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.') {
        return Err("Output name can only contain alphanumeric characters, hyphens, underscores, and dots".to_string());
    }
    
    Ok(())
}

fn main() {
    let args = Args::parse();
    
    // Validate output name
    if let Err(e) = validate_output_name(&args.output) {
        eprintln!("Error: Invalid output name: {}", e);
        process::exit(1);
    }
    
    // Read shader file contents to validate it's not empty
    let shader_content = match fs::read_to_string(&args.shader) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error: Failed to read shader file '{}': {}", args.shader, e);
            process::exit(1);
        }
    };
    
    if shader_content.trim().is_empty() {
        eprintln!("Error: Shader file '{}' is empty", args.shader);
        process::exit(1);
    }
    
    println!("Papyrust daemon starting...");
    println!("Output monitor: {}", args.output);
    println!("Shader file: {}", args.shader);
    println!("Shader file size: {} bytes", shader_content.len());
    
    // TODO: Implement actual wallpaper application logic
    println!("Successfully validated arguments. Daemon would now apply the shader to the specified output.");
}

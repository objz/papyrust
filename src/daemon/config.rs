use anyhow::Result;
use std::path::PathBuf;

use crate::Args;

#[derive(Debug, Clone)]
pub struct Config {
    pub socket_path: PathBuf,
    pub default_mode: ScalingMode,
    pub transitions_enabled: bool,
    pub transition_duration: u64,
    pub debug: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalingMode {
    Fill,   // Scale to fill, crop excess
    Fit,    // Scale to fit, add letterbox
    Center, // Center without scaling
    Tile,   // Tile the image
}

impl Config {
    pub fn new(args: Args) -> Result<Self> {
        let socket_path = if let Some(path) = args.socket {
            PathBuf::from(path)
        } else {
            let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
                .or_else(|_| std::env::var("TMPDIR"))
                .unwrap_or_else(|_| "/tmp".to_string());

            let display =
                std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-0".to_string());

            PathBuf::from(runtime_dir).join(format!("papyrust-{}.socket", display))
        };

        let default_mode = match args.mode.as_str() {
            "fill" => ScalingMode::Fill,
            "fit" => ScalingMode::Fit,
            "center" => ScalingMode::Center,
            "tile" => ScalingMode::Tile,
            _ => ScalingMode::Fill,
        };

        Ok(Config {
            socket_path,
            default_mode,
            transitions_enabled: !args.no_transitions,
            transition_duration: args.transition_duration,
            debug: args.debug,
        })
    }
}

use anyhow::{Result, anyhow};
use std::process::{Child, Command};
use tracing::{debug, info, warn};

pub struct AudioPlayer {
    child: Option<Child>,
    current_path: Option<String>,
}

impl AudioPlayer {
    pub fn new() -> Self {
        Self {
            child: None,
            current_path: None,
        }
    }

    pub fn play(&mut self, path: &str) -> Result<()> {
        self.stop()?;

        info!(
            event = "audio_player_starting",
            path = %path,
            "Starting ffplay for audio playback"
        );

        match Command::new("ffplay")
            .args(&[
                "-nodisp",
                "-autoexit",
                "-hide_banner",
                "-loglevel",
                "error",
                path,
            ])
            .spawn()
        {
            Ok(child) => {
                self.child = Some(child);
                self.current_path = Some(path.to_string());
                info!(
                    event = "audio_player_started",
                    path = %path,
                    "Successfully started ffplay for audio"
                );
                Ok(())
            }
            Err(e) => {
                warn!(
                    event = "audio_player_fail",
                    error = %e,
                    path = %path,
                    "Failed to start ffplay"
                );
                Err(anyhow!("Failed to start audio player: {}", e))
            }
        }
    }

    pub fn stop(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            debug!(event = "audio_player_stopping", "Stopping ffplay process");

            let _ = child.kill();
            let _ = child.wait();

            debug!(event = "audio_player_stopped", "Stopped ffplay process");
        }
        self.current_path = None;
        Ok(())
    }

    pub fn is_playing(&self) -> bool {
        self.child.is_some()
    }

    pub fn is_playing_path(&self, path: &str) -> bool {
        self.current_path.as_deref() == Some(path)
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

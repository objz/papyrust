use super::player::AudioPlayer;
use crate::media::MediaType;
use anyhow::Result;
use tracing::{debug, info};

pub struct AudioManager {
    player: AudioPlayer,
    muted: bool,
    global_mute: bool,
}

impl AudioManager {
    pub fn new(global_mute: bool) -> Self {
        info!(
            event = "audio_manager_init",
            global_mute, "Initializing audio manager"
        );

        Self {
            player: AudioPlayer::new(),
            muted: false,
            global_mute,
        }
    }

    pub fn handle_change(&mut self, media_type: &MediaType, media_mute: bool) -> Result<()> {
        match media_type {
            MediaType::Video { path, .. } => self.set_audio(path, media_mute),
            MediaType::Image { .. } | MediaType::Shader(_) => self.stop_audio(),
        }
    }

    pub fn set_audio(&mut self, path: &str, media_mute: bool) -> Result<()> {
        let effective_mute = self.global_mute || media_mute;

        info!(
            event = "audio_set_video",
            path = %path,
            global_mute = self.global_mute,
            media_mute = media_mute,
            effective_mute = effective_mute,
            "Setting video audio"
        );

        if effective_mute {
            if self.player.is_playing() {
                debug!(event = "audio_stop_muted", "Stopping audio due to mute");
                self.player.stop()?;
            }
            self.muted = true;
            return Ok(());
        }

        if self.player.is_playing_path(path) {
            debug!(
                event = "audio_already_playing",
                path = %path,
                "Audio already playing for this path"
            );
            return Ok(());
        }

        self.player.play(path)?;
        self.muted = false;
        Ok(())
    }

    pub fn stop_audio(&mut self) -> Result<()> {
        info!(event = "audio_stop", "Stopping all audio playback");
        self.player.stop()?;
        self.muted = false;
        Ok(())
    }

    pub fn is_playing(&self) -> bool {
        self.player.is_playing() && !self.muted
    }

    pub fn cleanup(&mut self) -> Result<()> {
        info!(event = "audio_cleanup", "Cleaning up audio manager");
        self.stop_audio()
    }
}

impl Drop for AudioManager {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

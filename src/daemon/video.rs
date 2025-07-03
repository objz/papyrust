use anyhow::{anyhow, Result};
use libmpv::Mpv;
use log::{debug, info, warn};
use mpv::Format;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct VideoPlayer {
    mpv: Arc<Mutex<Mpv>>,
    width: u32,
    height: u32,
    is_playing: bool,
    path: String,
}

impl VideoPlayer {
    pub async fn new(
        path: &str,
        width: u32,
        height: u32,
        loop_video: bool,
        audio: bool,
    ) -> Result<Self> {
        // Create and initialize mpv context
        let mut mpv = Mpv::new().map_err(|e| anyhow!("Failed to create mpv: {:?}", e))?;

        // Set options before initialization
        mpv.set_option("audio", Format::Flag, &audio)?;
        mpv.set_option("vo", Format::String, &"gpu")?; // use "libmpv" for advanced embedding, but "gpu" is simplest
        mpv.set_option("hwdec", Format::String, &"auto")?;
        mpv.set_option("keep-open", Format::Flag, &true)?;
        if loop_video {
            mpv.set_option("loop", Format::String, &"inf")?;
        }

        // (No explicit mpv_initialize required: mpv-rs handles this for you)

        // Load file
        mpv.command("loadfile", &[path])?;

        Ok(Self {
            mpv: Arc::new(Mutex::new(mpv)),
            width,
            height,
            is_playing: true,
            path: path.to_string(),
        })
    }

    pub async fn load_file(&mut self, path: &str) -> Result<()> {
        let mpv = self.mpv.clone();
        let mut mpv = mpv.lock().await;
        mpv.command("loadfile", &[path])?;
        self.is_playing = true;
        info!("Loaded video file: {}", path);
        Ok(())
    }

    pub async fn stop(&mut self) {
        let mpv = self.mpv.clone();
        let mut mpv = mpv.lock().await;
        let _ = mpv.command("stop", &[]);
        self.is_playing = false;
        debug!("Stopped video playback for: {}", self.path);
    }

    pub async fn pause(&mut self) -> Result<()> {
        let mpv = self.mpv.clone();
        let mut mpv = mpv.lock().await;
        mpv.set_property("pause", true)?;
        debug!("Paused video: {}", self.path);
        Ok(())
    }

    pub async fn resume(&mut self) -> Result<()> {
        let mpv = self.mpv.clone();
        let mut mpv = mpv.lock().await;
        mpv.set_property("pause", false)?;
        debug!("Resumed video: {}", self.path);
        Ok(())
    }

    pub async fn seek(&mut self, position: f64) -> Result<()> {
        let mpv = self.mpv.clone();
        let mut mpv = mpv.lock().await;
        mpv.command("seek", &[&position.to_string()])?;
        debug!("Seeked to position {} in video: {}", position, self.path);
        Ok(())
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    pub fn get_path(&self) -> &str {
        &self.path
    }

    pub async fn get_video_info(&self) -> Result<VideoInfo> {
        let mpv = self.mpv.clone();
        let mpv = mpv.lock().await;

        // Query width, height, duration, fps
        let width = mpv.get_property::<u32>("width")?.unwrap_or(self.width);
        let height = mpv.get_property::<u32>("height")?.unwrap_or(self.height);
        let duration = mpv.get_property::<f64>("duration")?.unwrap_or(0.0);
        let fps = mpv.get_property::<f64>("fps")?.unwrap_or(30.0);

        Ok(VideoInfo {
            width,
            height,
            duration,
            fps,
            path: self.path.clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub fps: f64,
    pub path: String,
}

use anyhow::Result;
use crate::media::MediaType;
use super::types::RenderContext;

/// Trait for Wayland surface operations
pub trait WaylandSurface {
    fn resize(&mut self, width: u32, height: u32) -> Result<()>;
    fn get_output_name(&self) -> &str;
}

/// Trait for media rendering operations
pub trait MediaRenderer {
    fn update_media(&mut self, media_type: MediaType, fps: u16) -> Result<()>;
    fn draw(&mut self, context: &RenderContext) -> Result<()>;
    fn has_new_frame(&self) -> bool;
}

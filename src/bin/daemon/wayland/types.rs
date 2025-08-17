use super::audio::fifo::FifoReader;
use khronos_egl as egl;
use wayland_client::protocol::wl_output;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutputId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceId(pub u32);

#[derive(Debug, Clone)]
pub struct DisplayConfig {
    pub width: u32,
    pub height: u32,
    pub scale: i32,
    pub transform: wl_output::Transform,
    pub logical_width: Option<u32>,
    pub logical_height: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct OutputInfo {
    pub output: wl_output::WlOutput,
    pub config: DisplayConfig,
    pub name: Option<String>,
}

pub struct RenderContext<'a> {
    pub width: i32,
    pub height: i32,
    pub fifo_reader: Option<&'a mut FifoReader>,
}

pub struct EglResources {
    pub display: egl::Display,
    pub surface: egl::Surface,
    pub context: egl::Context,
    pub config: egl::Config,
}

#[derive(Debug, Clone)]
pub struct WaylandConfig {
    pub fps: u16,
    pub layer_name: Option<String>,
}

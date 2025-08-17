use wayland_client::protocol::wl_output;
use khronos_egl as egl;
use super::audio::fifo::FifoReader;

/// Unique identifier for outputs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutputId(pub u32);

/// Unique identifier for surfaces  
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceId(pub u32);

/// Display configuration for an output
#[derive(Debug, Clone)]
pub struct DisplayConfig {
    pub width: u32,
    pub height: u32,
    pub scale: i32,
    pub transform: wl_output::Transform,
    pub logical_width: Option<u32>,
    pub logical_height: Option<u32>,
}

/// Information about a Wayland output
#[derive(Debug, Clone)]
pub struct OutputInfo {
    pub output: wl_output::WlOutput,
    pub config: DisplayConfig,
    pub name: Option<String>,
}

/// Rendering context passed to draw operations
pub struct RenderContext<'a> {
    pub width: i32,
    pub height: i32,
    pub fifo_reader: Option<&'a mut FifoReader>,
}

/// EGL resources for a surface
pub struct EglResources {
    pub display: egl::Display,
    pub surface: egl::Surface,
    pub context: egl::Context,
    pub config: egl::Config,
}

/// Configuration for the Wayland subsystem
#[derive(Debug, Clone)]
pub struct WaylandConfig {
    pub fps: u16,
    pub layer_name: Option<String>,
}

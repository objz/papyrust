use wayland_client::protocol::{wl_output, wl_compositor};
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_manager_v1;
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};
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
    pub transform: wl_output::Transform,
    pub fifo_reader: Option<&'a mut FifoReader>,
}

/// EGL resources for a surface
pub struct EglResources {
    pub display: egl::Display,
    pub surface: egl::Surface,
    pub context: egl::Context,
    pub config: egl::Config,
}

/// Protocol state containing global Wayland objects
pub struct ProtocolGlobals {
    pub compositor: Option<wl_compositor::WlCompositor>,
    pub layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    pub output_manager: Option<zxdg_output_manager_v1::ZxdgOutputManagerV1>,
}

/// Surface state tracking
pub struct SurfaceState {
    pub id: SurfaceId,
    pub output_id: OutputId,
    pub output_name: String,
    pub layer_surface: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    pub configured: bool,
}

/// Configuration for the Wayland subsystem
#[derive(Debug, Clone)]
pub struct WaylandConfig {
    pub fps: u16,
    pub layer_name: Option<String>,
    pub fifo_path: Option<String>,
    pub mute: bool,
    pub sharpening: f32,
}

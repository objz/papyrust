use tracing::{debug, info};
use std::collections::HashMap;
use wayland_client::protocol::{wl_compositor, wl_output, wl_region, wl_registry, wl_surface};
use wayland_client::{Connection, Dispatch, QueueHandle, Proxy};
use wayland_protocols::xdg::xdg_output::zv1::client::{zxdg_output_manager_v1, zxdg_output_v1};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

#[derive(Debug, Clone)]
pub struct OutputInfo {
    pub output: wl_output::WlOutput,
    pub width: i32,
    pub height: i32,
    pub name: Option<String>,
    pub transform: wl_output::Transform,
    pub scale: i32,
    pub logical_width: Option<u32>,
    pub logical_height: Option<u32>,
}

pub struct AppState {
    pub outputs: HashMap<u32, OutputInfo>,
    pub compositor: Option<wl_compositor::WlCompositor>,
    pub layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    pub output_manager: Option<zxdg_output_manager_v1::ZxdgOutputManagerV1>,
    pub configured_count: usize,
    pub total_surfaces: usize,
    pub layer_surface_configs: HashMap<u32, (u32, u32)>,
    pub surface_to_output: HashMap<u32, String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            outputs: HashMap::new(),
            compositor: None,
            layer_shell: None,
            output_manager: None,
            configured_count: 0,
            total_surfaces: 0,
            layer_surface_configs: HashMap::new(),
            surface_to_output: HashMap::new(),
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for AppState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<AppState>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => {
                debug!("Global: {} {} {}", name, interface, version);
                match interface.as_str() {
                    "wl_output" => {
                        let output =
                            registry.bind::<wl_output::WlOutput, _, _>(name, version, qh, name);
                        state.outputs.insert(
                            name,
                            OutputInfo {
                                output,
                                width: 0,
                                height: 0,
                                name: None,
                                transform: wl_output::Transform::Normal,
                                scale: 1,
                                logical_width: None,
                                logical_height: None,
                            },
                        );
                    }
                    "wl_compositor" => {
                        state.compositor =
                            Some(registry.bind::<wl_compositor::WlCompositor, _, _>(
                                name,
                                version,
                                qh,
                                (),
                            ));
                    }
                    "zwlr_layer_shell_v1" => {
                        state.layer_shell = Some(
                            registry.bind::<zwlr_layer_shell_v1::ZwlrLayerShellV1, _, _>(
                                name,
                                version,
                                qh,
                                (),
                            ),
                        );
                    }
                    "zxdg_output_manager_v1" => {
                        state.output_manager = Some(
                            registry.bind::<zxdg_output_manager_v1::ZxdgOutputManagerV1, _, _>(
                                name,
                                version,
                                qh,
                                (),
                            ),
                        );
                    }
                    _ => {}
                }
            }
            wl_registry::Event::GlobalRemove { name } => {
                state.outputs.remove(&name);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, u32> for AppState {
    fn event(
        state: &mut Self,
        _: &wl_output::WlOutput,
        event: wl_output::Event,
        id: &u32,
        _: &Connection,
        _: &QueueHandle<AppState>,
    ) {
        if let Some(info) = state.outputs.get_mut(id) {
            match event {
                wl_output::Event::Geometry { transform, .. } => {
                    info.transform = transform
                        .into_result()
                        .unwrap_or(wl_output::Transform::Normal);
                }
                wl_output::Event::Scale { factor } => {
                    info.scale = factor;
                }
                wl_output::Event::Mode {
                    flags,
                    width,
                    height,
                    ..
                } => {
                    if let Ok(m) = flags.into_result() {
                        if m.contains(wl_output::Mode::Current) {
                            info.width = width;
                            info.height = height;
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<zxdg_output_v1::ZxdgOutputV1, u32> for AppState {
    fn event(
        state: &mut Self,
        _: &zxdg_output_v1::ZxdgOutputV1,
        event: zxdg_output_v1::Event,
        output_id: &u32,
        _: &Connection,
        _: &QueueHandle<AppState>,
    ) {
        match event {
            zxdg_output_v1::Event::Name { name } => {
                if let Some(output_info) = state.outputs.get_mut(output_id) {
                    output_info.name = Some(name);
                }
            }
            zxdg_output_v1::Event::LogicalSize { width, height } => {
                if let Some(output_info) = state.outputs.get_mut(output_id) {
                    output_info.logical_width = Some(width as u32);
                    output_info.logical_height = Some(height as u32);
                    debug!("Output {} logical size: {}x{}", output_info.name.as_deref().unwrap_or("unknown"), width, height);
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, Option<String>> for AppState {
    fn event(
        state: &mut Self,
        surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        output_name: &Option<String>,
        _: &Connection,
        _: &QueueHandle<AppState>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                surface.ack_configure(serial);
                let surface_id = surface.id().protocol_id();
                let output_name = output_name.clone().unwrap_or_else(|| format!("unknown-{}", surface_id));
                
                info!("Layer surface {} (output {}) configured: {}x{}", surface_id, output_name, width, height);
                
                state.layer_surface_configs.insert(surface_id, (width, height));
                state.surface_to_output.insert(surface_id, output_name);
                state.configured_count += 1;
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &wl_compositor::WlCompositor,
        _: wl_compositor::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppState>,
    ) {
    }
}

impl Dispatch<zwlr_layer_shell_v1::ZwlrLayerShellV1, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &zwlr_layer_shell_v1::ZwlrLayerShellV1,
        _: zwlr_layer_shell_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppState>,
    ) {
    }
}

impl Dispatch<zxdg_output_manager_v1::ZxdgOutputManagerV1, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &zxdg_output_manager_v1::ZxdgOutputManagerV1,
        _: zxdg_output_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppState>,
    ) {
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &wl_surface::WlSurface,
        _: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppState>,
    ) {
    }
}

impl Dispatch<wl_region::WlRegion, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &wl_region::WlRegion,
        _: wl_region::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppState>,
    ) {
    }
}

use log::debug;
use std::collections::HashMap;
use wayland_client::protocol::{wl_compositor, wl_output, wl_region, wl_registry, wl_surface};
use wayland_client::{Connection, Dispatch, QueueHandle};
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
}

pub struct AppState {
    pub outputs: HashMap<u32, OutputInfo>,
    pub compositor: Option<wl_compositor::WlCompositor>,
    pub layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    pub output_manager: Option<zxdg_output_manager_v1::ZxdgOutputManagerV1>,
    pub configured_count: usize,
    pub total_surfaces: usize,
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
            _ => {}
        }
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for AppState {
    fn event(
        state: &mut Self,
        surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppState>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width: _,
                height: _,
            } => {
                surface.ack_configure(serial);
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



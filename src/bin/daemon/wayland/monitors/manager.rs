use std::collections::HashMap;
use anyhow::Result;
use khronos_egl as egl;
use wayland_client::{Connection, QueueHandle};
use wayland_client::protocol::wl_compositor;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1;
use crate::media::MediaType;
use crate::wayland::rendering::surface::WaylandSurface;
use crate::wayland::types::{OutputInfo, RenderContext};
use crate::wayland::protocol::events::AppState;
use crate::wayland::traits::WaylandSurface as WaylandSurfaceTrait;
use crate::wayland::audio::FifoReader;

pub struct MonitorManager {
    surfaces: HashMap<String, WaylandSurface>,
    egl_instance: egl::Instance<egl::Static>,
}

impl MonitorManager {
    pub fn new() -> Self {
        Self {
            surfaces: HashMap::new(),
            egl_instance: egl::Instance::new(egl::Static),
        }
    }

    pub fn create_surface(
        &mut self,
        output_info: &OutputInfo,
        compositor: &wl_compositor::WlCompositor,
        layer_shell: &zwlr_layer_shell_v1::ZwlrLayerShellV1,
        layer_name: Option<&str>,
        media_type: MediaType,
        conn: &Connection,
        qh: &QueueHandle<AppState>,
        fps: u16,
    ) -> Result<()> {
        let surface = WaylandSurface::new(
            output_info,
            compositor,
            layer_shell,
            layer_name,
            media_type,
            &self.egl_instance,
            conn,
            qh,
            fps,
        )?;

        let output_name = surface.get_output_name().to_string();
        self.surfaces.insert(output_name, surface);
        Ok(())
    }

    pub fn surfaces_mut(&mut self) -> impl Iterator<Item = &mut WaylandSurface> {
        self.surfaces.values_mut()
    }

    pub fn update_media(&mut self, target_monitors: Option<&[String]>, media_type: MediaType, fps: u16) -> Result<()> {
        match target_monitors {
            None => {
                tracing::info!(
                    event = "media_update_all",
                    ?media_type,
                    monitors = self.surfaces.len(),
                    available_monitors = ?self.surfaces.keys().collect::<Vec<_>>(),
                    "Updating media on all monitors"
                );
                for (monitor_name, surface) in &mut self.surfaces {
                    tracing::debug!(
                        event = "media_update_monitor",
                        monitor = %monitor_name,
                        "Applying media to monitor"
                    );
                    surface.renderer.update_media(media_type.clone(), fps)?;
                }
            }
            Some(target_names) => {
                tracing::info!(
                    event = "media_update_targeted",
                    targets = ?target_names,
                    ?media_type,
                    available_monitors = ?self.surfaces.keys().collect::<Vec<_>>(),
                    "Updating media on specific monitors"
                );
                
                let mut found_monitors = Vec::new();
                let mut missing_monitors = Vec::new();
                
                for target_name in target_names {
                    if let Some(surface) = self.surfaces.get_mut(target_name) {
                        tracing::debug!(
                            event = "media_update_monitor",
                            monitor = %target_name,
                            "Applying media to target monitor"
                        );
                        surface.renderer.update_media(media_type.clone(), fps)?;
                        found_monitors.push(target_name);
                    } else {
                        missing_monitors.push(target_name);
                    }
                }
                
                if !missing_monitors.is_empty() {
                    tracing::warn!(
                        event = "monitors_not_found",
                        missing = ?missing_monitors,
                        found = ?found_monitors,
                        available = ?self.surfaces.keys().collect::<Vec<_>>(),
                        "Some target monitors were not found"
                    );
                }
                
                if found_monitors.is_empty() {
                    tracing::error!(
                        event = "no_monitors_updated",
                        targets = ?target_names,
                        available = ?self.surfaces.keys().collect::<Vec<_>>(),
                        "No target monitors were found - no media was updated"
                    );
                }
            }
        }
        Ok(())
    }

    pub fn set_swap_intervals(&self, has_video: bool, fps: u16) -> Result<()> {
        for surface in self.surfaces.values() {
            if has_video {
                self.egl_instance.swap_interval(surface.egl_resources.display, 1)?;
            } else {
                let interval = if fps == 0 { 1 } else { 0 };
                self.egl_instance.swap_interval(surface.egl_resources.display, interval)?;
            }
        }
        Ok(())
    }

    pub fn render_all(&mut self, mut fifo_reader: Option<&mut FifoReader>) -> Result<bool> {
        let mut any_updated = false;
        
        let surface_names: Vec<String> = self.surfaces.keys().cloned().collect();
        
        for surface_name in surface_names {
            if let Some(surface) = self.surfaces.get_mut(&surface_name) {
                self.egl_instance.make_current(
                    surface.egl_resources.display,
                    Some(surface.egl_resources.surface),
                    Some(surface.egl_resources.surface),
                    Some(surface.egl_resources.context),
                )?;

                if surface.renderer.has_new_frame() {
                    any_updated = true;
                }

                let mut surface_context = RenderContext {
                    width: surface.current_width as i32,
                    height: surface.current_height as i32,
                    fifo_reader: fifo_reader.as_deref_mut(),
                };

                surface.renderer.draw(&mut surface_context)?;
                
                self.egl_instance.swap_buffers(surface.egl_resources.display, surface.egl_resources.surface)?;
                
                tracing::trace!(
                    event = "surface_rendered",
                    monitor = %surface_name,
                    width = surface.current_width,
                    height = surface.current_height,
                    "Successfully rendered frame"
                );
            }
        }
        
        Ok(any_updated)
    }

    pub fn len(&self) -> usize {
        self.surfaces.len()
    }
}

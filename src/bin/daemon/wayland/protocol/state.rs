use std::collections::HashMap;
use wayland_client::{Connection, QueueHandle};
use crate::wayland::types::{OutputId, OutputInfo, ProtocolGlobals, SurfaceState, SurfaceId};

/// Manages the overall protocol state
pub struct ProtocolState {
    pub outputs: HashMap<OutputId, OutputInfo>,
    pub surfaces: HashMap<SurfaceId, SurfaceState>, 
    pub globals: ProtocolGlobals,
    pub configured_count: usize,
    pub total_surfaces: usize,
    pub layer_surface_configs: HashMap<u32, (u32, u32)>,
    pub surface_to_output: HashMap<u32, String>,
}

impl ProtocolState {
    pub fn new() -> Self {
        Self {
            outputs: HashMap::new(),
            surfaces: HashMap::new(),
            globals: ProtocolGlobals {
                compositor: None,
                layer_shell: None,
                output_manager: None,
            },
            configured_count: 0,
            total_surfaces: 0,
            layer_surface_configs: HashMap::new(),
            surface_to_output: HashMap::new(),
        }
    }

    pub fn add_output(&mut self, id: OutputId, info: OutputInfo) {
        self.outputs.insert(id, info);
    }

    pub fn remove_output(&mut self, id: OutputId) {
        self.outputs.remove(&id);
    }

    pub fn get_output(&self, id: OutputId) -> Option<&OutputInfo> {
        self.outputs.get(&id)
    }

    pub fn get_output_mut(&mut self, id: OutputId) -> Option<&mut OutputInfo> {
        self.outputs.get_mut(&id)
    }

    pub fn add_surface(&mut self, surface: SurfaceState) {
        self.surfaces.insert(surface.id, surface);
        self.total_surfaces += 1;
    }

    pub fn get_surface(&self, id: SurfaceId) -> Option<&SurfaceState> {
        self.surfaces.get(&id)
    }

    pub fn get_surface_mut(&mut self, id: SurfaceId) -> Option<&mut SurfaceState> {
        self.surfaces.get_mut(&id)
    }

    pub fn configure_surface(&mut self, id: SurfaceId, width: u32, height: u32) {
        if let Some(surface) = self.surfaces.get_mut(&id) {
            surface.configured = true;
            self.configured_count += 1;
        }
        self.layer_surface_configs.insert(id.0, (width, height));
    }

    pub fn is_ready(&self) -> bool {
        self.configured_count >= self.total_surfaces && self.total_surfaces > 0
    }

    pub fn bind_globals(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<crate::wayland::protocol::events::AppState>,
    ) -> anyhow::Result<()> {
        let _registry = conn.display().get_registry(qh, ());
        Ok(())
    }
}

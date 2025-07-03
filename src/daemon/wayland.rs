use anyhow::Result;
use log::{error, info};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::wlr_layer::{
        Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
    },
    shm::{Shm, ShmHandler},
};
use std::collections::HashMap;
use tokio::sync::mpsc;
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_buffer, wl_output, wl_shm_pool, wl_surface},
    Connection, QueueHandle,
};

use crate::{config::Config, ipc::WaylandCommand, renderer::Renderer};

pub struct WaylandClient {
    conn: Connection,
    registry_state: RegistryState,
    output_state: OutputState,
    compositor_state: CompositorState,
    shm: Shm,
    layer_shell: LayerShell,
    outputs: HashMap<String, OutputInfo>,
    surfaces: HashMap<String, SurfaceInfo>,
    renderer: Renderer,
    command_receiver: mpsc::Receiver<WaylandCommand>,
    command_sender: mpsc::Sender<WaylandCommand>,
}

#[derive(Debug, Clone)]
struct OutputInfo {
    output: wl_output::WlOutput,
    name: String,
    width: i32,
    height: i32,
    scale: i32,
}

#[derive(Debug)]
struct SurfaceInfo {
    surface: wl_surface::WlSurface,
    layer_surface: LayerSurface,
    width: u32,
    height: u32,
    current_content: Option<String>,
}

impl WaylandClient {
    pub async fn new(config: &Config) -> Result<Self> {
        let conn = Connection::connect_to_env()?;
        let (globals, mut event_queue) = registry_queue_init(&conn)?;
        let qh = event_queue.handle();

        let registry_state = RegistryState::new(&globals);
        let output_state = OutputState::new(&globals, &qh);
        let compositor_state = CompositorState::bind(&globals, &qh)?;
        let shm = Shm::bind(&globals, &qh)?;
        let layer_shell = LayerShell::bind(&globals, &qh)?;

        let renderer = Renderer::new(config.clone())?;
        let (command_sender, command_receiver) = mpsc::channel(32);

        let mut client = Self {
            conn,
            registry_state,
            output_state,
            compositor_state,
            shm,
            layer_shell,
            outputs: HashMap::new(),
            surfaces: HashMap::new(),
            renderer,
            command_receiver,
            command_sender,
        };

        // Process initial events to discover outputs
        event_queue.blocking_dispatch(&mut client)?;

        // Create surfaces for all outputs
        let output_names: Vec<String> = client.outputs.keys().cloned().collect();
        for name in output_names {
            if let Some(output_info) = client.outputs.get(&name) {
                let output_info = output_info.clone();
                client.create_surface_for_output(&name, &output_info, &qh)?;
            }
        }

        Ok(client)
    }

    pub fn get_sender(&self) -> mpsc::Sender<WaylandCommand> {
        self.command_sender.clone()
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut event_queue = self.conn.new_event_queue();
        let qh = event_queue.handle();

        loop {
            tokio::select! {
                // Handle IPC commands
                Some(command) = self.command_receiver.recv() => {
                    if let Err(e) = self.handle_command(command, &qh).await {
                        error!("Error handling command: {}", e);
                    }
                }

                else => break,
            }

            // Process Wayland events
            event_queue.blocking_dispatch(self)?;
        }

        Ok(())
    }

    fn create_surface_for_output(
        &mut self,
        name: &str,
        output_info: &OutputInfo,
        qh: &QueueHandle<Self>,
    ) -> Result<()> {
        let surface = self.compositor_state.create_surface(qh);

        let layer_surface = self.layer_shell.create_layer_surface(
            qh,
            surface.clone(),
            Layer::Background,
            Some("papyrust-wallpaper"),
            Some(&output_info.output),
        );

        layer_surface.set_size(output_info.width as u32, output_info.height as u32);
        layer_surface.set_anchor(Anchor::all());
        layer_surface.set_exclusive_zone(-1);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);

        surface.commit();

        let surface_info = SurfaceInfo {
            surface,
            layer_surface,
            width: output_info.width as u32,
            height: output_info.height as u32,
            current_content: None,
        };

        self.surfaces.insert(name.to_string(), surface_info);
        info!("Created surface for output: {}", name);

        Ok(())
    }

    async fn handle_command(
        &mut self,
        command: WaylandCommand,
        qh: &QueueHandle<Self>,
    ) -> Result<()> {
        use WaylandCommand::*;

        match command {
            SetImage { path, output, mode } => {
                self.set_image(path, output, mode, qh).await?;
            }
            SetVideo {
                path,
                output,
                r#loop,
                audio,
            } => {
                self.set_video(path, output, r#loop, audio, qh).await?;
            }
            SetGif { path, output, mode } => {
                self.set_gif(path, output, mode, qh).await?;
            }
            Query { output } => {
                self.query_status(output);
            }
            Stop { output } => {
                self.stop_content(output, qh).await?;
            }
        }

        Ok(())
    }

    async fn set_image(
        &mut self,
        path: String,
        output: String,
        mode: crate::config::ScalingMode,
        qh: &QueueHandle<Self>,
    ) -> Result<()> {
        let targets = self.resolve_output_targets(&output);

        for target in targets {
            if let Some(surface_info) = self.surfaces.get_mut(&target) {
                let buffer = self
                    .renderer
                    .render_image(
                        &path,
                        surface_info.width,
                        surface_info.height,
                        mode,
                        &self.shm.wl_shm(),
                        qh,
                    )
                    .await?;

                surface_info.surface.attach(Some(&buffer), 0, 0);
                surface_info.surface.damage_buffer(
                    0,
                    0,
                    surface_info.width as i32,
                    surface_info.height as i32,
                );
                surface_info.surface.commit();
                surface_info.current_content = Some(path.clone());

                info!("Set image {} on output {}", path, target);
            }
        }

        Ok(())
    }

    async fn set_video(
        &mut self,
        path: String,
        output: String,
        r#loop: bool,
        audio: bool,
        qh: &QueueHandle<Self>,
    ) -> Result<()> {
        let targets = self.resolve_output_targets(&output);

        for target in targets {
            if let Some(surface_info) = self.surfaces.get_mut(&target) {
                self.renderer
                    .start_video(
                        &path,
                        &target,
                        surface_info.width,
                        surface_info.height,
                        r#loop,
                        audio,
                        &self.shm.wl_shm(),
                        qh,
                        surface_info.surface.clone(),
                    )
                    .await?;

                surface_info.current_content = Some(path.clone());
                info!("Started video {} on output {}", path, target);
            }
        }

        Ok(())
    }

    async fn set_gif(
        &mut self,
        path: String,
        output: String,
        mode: crate::config::ScalingMode,
        qh: &QueueHandle<Self>,
    ) -> Result<()> {
        let targets = self.resolve_output_targets(&output);

        for target in targets {
            if let Some(surface_info) = self.surfaces.get_mut(&target) {
                self.renderer
                    .start_gif(
                        &path,
                        &target,
                        surface_info.width,
                        surface_info.height,
                        mode,
                        &self.shm.wl_shm(),
                        qh,
                        surface_info.surface.clone(),
                    )
                    .await?;

                surface_info.current_content = Some(path.clone());
                info!("Started GIF {} on output {}", path, target);
            }
        }

        Ok(())
    }

    fn query_status(&self, output: String) {
        let targets = self.resolve_output_targets(&output);

        for target in targets {
            if let Some(surface_info) = self.surfaces.get(&target) {
                let content = surface_info.current_content.as_deref().unwrap_or("none");
                info!(
                    "Output {}: {} ({}x{})",
                    target, content, surface_info.width, surface_info.height
                );
            }
        }
    }

    async fn stop_content(&mut self, output: String, qh: &QueueHandle<Self>) -> Result<()> {
        let targets = self.resolve_output_targets(&output);

        for target in targets {
            if let Some(surface_info) = self.surfaces.get_mut(&target) {
                // Stop any ongoing animations
                self.renderer.stop_content(&target);

                // Clear the surface with black
                let buffer = self
                    .renderer
                    .create_solid_color_buffer(
                        surface_info.width,
                        surface_info.height,
                        [0, 0, 0, 255],
                        &self.shm.wl_shm(),
                        qh,
                    )
                    .await?;

                surface_info.surface.attach(Some(&buffer), 0, 0);
                surface_info.surface.damage_buffer(
                    0,
                    0,
                    surface_info.width as i32,
                    surface_info.height as i32,
                );
                surface_info.surface.commit();
                surface_info.current_content = None;

                info!("Stopped content on output {}", target);
            }
        }

        Ok(())
    }

    fn resolve_output_targets(&self, output: &str) -> Vec<String> {
        if output == "all" {
            self.outputs.keys().cloned().collect()
        } else {
            vec![output.to_string()]
        }
    }
}

// Implement required traits for smithay-client-toolkit
impl CompositorHandler for WaylandClient {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
        // Handle scale factor changes
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
        // Handle transform changes
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        // Handle frame callbacks
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
        // Handle surface entering output
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
        // Handle surface leaving output
    }
}

impl OutputHandler for WaylandClient {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        // Handle new output
        if let Some(info) = self.output_state.info(&output) {
            let output_info = OutputInfo {
                output: output.clone(),
                name: info
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("output-{}", self.outputs.len())),
                width: info.logical_size.unwrap_or((1920, 1080)).0,
                height: info.logical_size.unwrap_or((1920, 1080)).1,
                scale: info.scale_factor,
            };

            let name = output_info.name.clone();
            self.outputs.insert(name.clone(), output_info.clone());

            if let Err(e) = self.create_surface_for_output(&name, &output_info, qh) {
                error!("Failed to create surface for output {}: {}", name, e);
            }
        }
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        // Handle output updates
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        // Handle output destruction
        self.outputs.retain(|_, info| info.output != output);
    }
}

impl LayerShellHandler for WaylandClient {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        // Handle layer surface closure
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        _configure: smithay_client_toolkit::shell::wlr_layer::LayerSurfaceConfigure,
        _serial: u32,
    ) {
        // Handle layer surface configuration
    }
}

impl ShmHandler for WaylandClient {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for WaylandClient {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState];
}

// Add dispatch implementations for buffer and shm pool
impl wayland_client::Dispatch<wl_shm_pool::WlShmPool, ()> for WaylandClient {
    fn event(
        _: &mut Self,
        _: &wl_shm_pool::WlShmPool,
        _: wl_shm_pool::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Handle shm pool events
    }
}

impl wayland_client::Dispatch<wl_buffer::WlBuffer, ()> for WaylandClient {
    fn event(
        _: &mut Self,
        _: &wl_buffer::WlBuffer,
        _: wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Handle buffer events
    }
}

delegate_compositor!(WaylandClient);
delegate_output!(WaylandClient);
delegate_shm!(WaylandClient);
delegate_layer!(WaylandClient);
delegate_registry!(WaylandClient);

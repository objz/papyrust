use anyhow::{Result, anyhow};
use khronos_egl as egl;
use wayland_client::protocol::wl_compositor;
use wayland_client::{Connection, Proxy, QueueHandle};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};
use crate::media::MediaType;
use crate::wayland::rendering::MediaRenderer;
use crate::wayland::types::{OutputInfo, EglResources, SurfaceId};
use crate::wayland::protocol::events::AppState;

pub struct WaylandSurface {
    pub egl_resources: EglResources,
    pub renderer: MediaRenderer,
    pub output_info: OutputInfo,
    pub egl_window: wayland_egl::WlEglSurface,
    pub current_width: u32,
    pub current_height: u32,
    pub surface_id: SurfaceId,
    pub configured: bool,
    pub output_name: String,
}

impl WaylandSurface {
    pub fn new(
        output_info: &OutputInfo,
        compositor: &wl_compositor::WlCompositor,
        layer_shell: &zwlr_layer_shell_v1::ZwlrLayerShellV1,
        layer_name: Option<&str>,
        media_type: MediaType,
        egl_instance: &egl::Instance<egl::Static>,
        conn: &Connection,
        qh: &QueueHandle<AppState>,
        fps: u16,
    ) -> Result<Self> {
        let surface = compositor.create_surface(qh, ());
        let input_region = compositor.create_region(qh, ());
        surface.set_input_region(Some(&input_region));

        let layer = match layer_name {
            Some("top") => zwlr_layer_shell_v1::Layer::Top,
            Some("bottom") => zwlr_layer_shell_v1::Layer::Bottom,
            Some("overlay") => zwlr_layer_shell_v1::Layer::Overlay,
            Some("background") | None => zwlr_layer_shell_v1::Layer::Background,
            _ => zwlr_layer_shell_v1::Layer::Background,
        };

        let layer_surface = layer_shell.get_layer_surface(
            &surface,
            Some(&output_info.output),
            layer,
            "papyrust-daemon".to_string(),
            qh,
            output_info.name.clone(),
        );

        let surface_id = SurfaceId(layer_surface.id().protocol_id());
        let output_name = output_info
            .name
            .clone()
            .unwrap_or_else(|| format!("unknown-{}", surface_id.0));

        tracing::info!(
            event = "layer_surface_create",
            output = %output_name,
            surface_id = surface_id.0,
            layer = ?layer_name.unwrap_or("background"),
            "Created layer surface"
        );

        layer_surface.set_exclusive_zone(-1);
        layer_surface.set_anchor(
            zwlr_layer_surface_v1::Anchor::Top
                | zwlr_layer_surface_v1::Anchor::Left
                | zwlr_layer_surface_v1::Anchor::Right
                | zwlr_layer_surface_v1::Anchor::Bottom,
        );

        surface.commit();

        let egl_resources = Self::create_egl_resources(egl_instance, conn)?;
        let initial_width = 100;
        let initial_height = 100;

        let egl_window = wayland_egl::WlEglSurface::new(surface.id(), initial_width, initial_height)
            .map_err(|e| anyhow!("Failed to create wl_egl_window: {e}"))?;

        let egl_surface = unsafe {
            egl_instance.create_window_surface(
                egl_resources.display,
                egl_resources.config,
                egl_window.ptr() as *mut _,
                Some(&[egl::NONE]),
            )?
        };

        egl_instance.make_current(
            egl_resources.display,
            Some(egl_surface),
            Some(egl_surface),
            Some(egl_resources.context),
        )?;

        tracing::debug!(
            event = "egl_ready",
            output = %output_name,
            width = initial_width,
            height = initial_height,
            "EGL surface/context ready"
        );

        let renderer = MediaRenderer::new(media_type, fps)?;

        Ok(Self {
            egl_resources: EglResources {
                display: egl_resources.display,
                surface: egl_surface,
                context: egl_resources.context,
                config: egl_resources.config,
            },
            renderer,
            output_info: output_info.clone(),
            egl_window,
            current_width: initial_width as u32,
            current_height: initial_height as u32,
            surface_id,
            configured: false,
            output_name,
        })
    }

    fn create_egl_resources(
        egl_instance: &egl::Instance<egl::Static>,
        conn: &Connection,
    ) -> Result<EglResources> {
        let display_ptr = conn.display().id().as_ptr();
        let egl_display = unsafe { egl_instance.get_display(display_ptr as *mut _) }
            .ok_or_else(|| anyhow!("Failed to get EGL display for Wayland connection"))?;
        let _version = egl_instance.initialize(egl_display)?;

        egl_instance.bind_api(egl::OPENGL_ES_API)?;

        let config_attribs = [
            egl::SURFACE_TYPE,
            egl::WINDOW_BIT,
            egl::RENDERABLE_TYPE,
            egl::OPENGL_ES2_BIT,
            egl::RED_SIZE,
            8,
            egl::GREEN_SIZE,
            8,
            egl::BLUE_SIZE,
            8,
            egl::ALPHA_SIZE,
            8,
            egl::NONE,
        ];

        let mut configs = Vec::with_capacity(1);
        egl_instance.choose_config(egl_display, &config_attribs, &mut configs)?;
        let config = configs
            .first()
            .ok_or_else(|| anyhow!("No suitable EGL config"))?;

        let context_attribs = [
            egl::CONTEXT_MAJOR_VERSION,
            2,
            egl::CONTEXT_MINOR_VERSION,
            0,
            egl::NONE,
        ];
        let context = egl_instance.create_context(egl_display, *config, None, &context_attribs)?;

        Ok(EglResources {
            display: egl_display,
            surface: unsafe { egl::Surface::from_ptr(std::ptr::null_mut()) },
            context,
            config: *config,
        })
    }
}

impl crate::wayland::traits::WaylandSurface for WaylandSurface {
    fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        if self.current_width != width || self.current_height != height {
            tracing::info!(
                event = "surface_resize",
                output = %self.output_name,
                surface_id = self.surface_id.0,
                from_width = self.current_width,
                from_height = self.current_height,
                to_width = width,
                to_height = height,
                "Applying surface resize"
            );
            self.egl_window.resize(width as i32, height as i32, 0, 0);
            self.current_width = width;
            self.current_height = height;
            self.configured = true;
        } else {
            tracing::debug!(
                event = "surface_resize_skipped",
                output = %self.output_name,
                width,
                height,
                "Resize skipped (dimensions unchanged)"
            );
        }
        Ok(())
    }

    fn get_output_name(&self) -> &str {
        &self.output_name
    }
}

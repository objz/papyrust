use anyhow::{anyhow, Result};
use khronos_egl as egl;
use wayland_client::protocol::{wl_compositor};
use wayland_client::{Connection, Proxy, QueueHandle};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1};

use crate::media::{
    MediaType
};
use crate::wayland::renderer::MediaRenderer;
use crate::wayland::state::{AppState, OutputInfo};



pub struct MonitorState {
    pub egl_display: egl::Display,
    pub egl_surface: egl::Surface,
    pub egl_context: egl::Context,
    pub renderer: MediaRenderer,
    pub output_info: OutputInfo,
}


pub fn create_monitor_state(
    output_info: &OutputInfo,
    compositor: &wl_compositor::WlCompositor,
    layer_shell: &zwlr_layer_shell_v1::ZwlrLayerShellV1,
    layer_name: Option<&str>,
    media_type: MediaType,
    egl_instance: &egl::Instance<egl::Static>,
    conn: &Connection,
    qh: &QueueHandle<AppState>,
) -> Result<MonitorState> {
    let surface = compositor.create_surface(qh, ());

    let input_region = compositor.create_region(qh, ());
    let render_region = compositor.create_region(qh, ());

    render_region.add(0, 0, output_info.width, output_info.height);

    surface.set_opaque_region(Some(&render_region));
    surface.set_input_region(Some(&input_region));

    surface.set_buffer_transform(output_info.transform);
    surface.set_buffer_scale(output_info.scale);

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
        (),
    );

    layer_surface.set_exclusive_zone(-1);

    layer_surface.set_size(output_info.width as u32, output_info.height as u32);
    surface.commit();

    let display_ptr = conn.display().id().as_ptr();
    let egl_display = unsafe { egl_instance.get_display(display_ptr as *mut _) }
        .ok_or_else(|| anyhow!("Failed to get EGL display for Wayland connection"))?;

    let _version = egl_instance.initialize(egl_display)?;

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

    let egl_surface_wrapper =
        wayland_egl::WlEglSurface::new(surface.id(), output_info.width, output_info.height)?;

    let egl_surface = unsafe {
        egl_instance.create_window_surface(
            egl_display,
            *config,
            egl_surface_wrapper.ptr() as *mut _,
            Some(&[egl::NONE]),
        )?
    };

    egl_instance.make_current(
        egl_display,
        Some(egl_surface),
        Some(egl_surface),
        Some(context),
    )?;

    let renderer = MediaRenderer::new(media_type)?;

    Ok(MonitorState {
        egl_display,
        egl_surface,
        egl_context: context,
        renderer,
        output_info: output_info.clone(),
    })
}

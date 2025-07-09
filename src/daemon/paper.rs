use anyhow::{anyhow, Result};
use khronos_egl as egl;
use log::{debug, info};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::mpsc::Receiver;
use wayland_client::protocol::{wl_compositor, wl_output, wl_region, wl_registry, wl_surface};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::xdg::xdg_output::zv1::client::{zxdg_output_manager_v1, zxdg_output_v1};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use crate::gl_bindings as gl;
use crate::media::{
    default_shader, load_shader, vertex_shader, ImageLoader, MediaType, VideoDecoder,
};
use crate::utils;

const N_SAMPLES: usize = 44100 / 25;

#[derive(Debug)]
struct OutputInfo {
    id: u32,
    output: wl_output::WlOutput,
    width: i32,
    height: i32,
    name: Option<String>,
}

#[derive(Debug)]
struct StereoSample {
    left: Vec<i16>,
    right: Vec<i16>,
}

impl StereoSample {
    fn new() -> Self {
        Self {
            left: vec![0; N_SAMPLES],
            right: vec![0; N_SAMPLES],
        }
    }
}

struct AppState {
    outputs: HashMap<u32, OutputInfo>,
    compositor: Option<wl_compositor::WlCompositor>,
    layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    output_manager: Option<zxdg_output_manager_v1::ZxdgOutputManagerV1>,
    target_output: Option<OutputInfo>,
    monitor_name: String,
    configured: bool,
}

impl AppState {
    fn new(monitor_name: String) -> Self {
        Self {
            outputs: HashMap::new(),
            compositor: None,
            layer_shell: None,
            output_manager: None,
            target_output: None,
            monitor_name,
            configured: false,
        }
    }
}

struct FifoReader {
    fd: RawFd,
}

impl FifoReader {
    fn new(fifo_path: &str) -> Result<Self> {
        use std::os::unix::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(fifo_path)?;

        Ok(Self {
            fd: file.as_raw_fd(),
        })
    }

    fn read_sample(&mut self) -> Result<Option<StereoSample>> {
        let mut buffer = vec![0u8; N_SAMPLES * 4];

        let bytes_read = unsafe {
            libc::read(
                self.fd,
                buffer.as_mut_ptr() as *mut libc::c_void,
                buffer.len(),
            )
        };

        if bytes_read < 0 {
            let errno = unsafe { *libc::__errno_location() };
            if errno == libc::EAGAIN || errno == libc::EWOULDBLOCK {
                return Ok(None);
            }
            return Err(anyhow!("Failed to read from FIFO: {}", errno));
        }

        if bytes_read == 0 {
            return Ok(None);
        }

        let samples_read = bytes_read as usize / 4;
        let mut stereo = StereoSample::new();

        for i in 0..samples_read.min(N_SAMPLES / 2) {
            let base = i * 4;
            if base + 3 < buffer.len() {
                stereo.left[i] = i16::from_le_bytes([buffer[base], buffer[base + 1]]);
                stereo.right[i] = i16::from_le_bytes([buffer[base + 2], buffer[base + 3]]);
            }
        }

        Ok(Some(stereo))
    }
}

struct MediaRenderer {
    shader_program: u32,
    media_texture: Option<u32>,
    video_decoder: Option<VideoDecoder>,
    _vbo: u32,
    _ebo: u32,
    start_time: u64,
    media_type: MediaType,
}

impl MediaRenderer {
    fn new(media_type: MediaType) -> Result<Self> {
        eprintln!("Creating MediaRenderer with type: {:?}", media_type);

        let start_time = utils::get_time_millis();

        unsafe {
            gl::load_with(|s| {
                let c_str = CString::new(s).unwrap();
                let proc_addr = match CStr::from_bytes_with_nul(b"eglGetProcAddress\0") {
                    Ok(name) => libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()),
                    Err(_) => std::ptr::null_mut(),
                };
                if proc_addr.is_null() {
                    std::ptr::null()
                } else {
                    let get_proc_addr: extern "C" fn(*const i8) -> *const std::ffi::c_void =
                        std::mem::transmute(proc_addr);
                    get_proc_addr(c_str.as_ptr())
                }
            });

            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
        }

        let (shader_program, media_texture, video_decoder) =
            if media_type == MediaType::Shader("default".to_string()) {
                let program = Self::default_shader()?;
                (program, None, None)
            } else {
                match &media_type {
                    MediaType::Shader(shader_path) => {
                        let program = Self::create_pure_shader(shader_path)?;
                        (program, None, None)
                    }
                    MediaType::Image { path, shader } => {
                        let texture = ImageLoader::load_texture(path)?;
                        let program = if let Some(shader_path) = shader {
                            Self::create_media_shader(shader_path)?
                        } else {
                            Self::create_default_shader()?
                        };
                        (program, Some(texture), None)
                    }
                    MediaType::Video { path, shader } => {
                        let decoder = VideoDecoder::new(path)?;
                        let texture = decoder.texture();
                        let program = if let Some(shader_path) = shader {
                            Self::create_media_shader(shader_path)?
                        } else {
                            Self::create_default_shader()?
                        };
                        (program, Some(texture), Some(decoder))
                    }
                }
            };

        let (vbo, ebo) = Self::setup_geometry()?;

        Ok(Self {
            shader_program,
            media_texture,
            video_decoder,
            _vbo: vbo,
            _ebo: ebo,
            start_time,
            media_type,
        })
    }

    fn default_shader() -> Result<u32> {
        let vert_source = r#"
            #version 100
            attribute highp vec2 datIn;
            attribute highp vec2 texIn;
            varying vec2 texCoords;
            void main() {
                texCoords = texIn;
                gl_Position = vec4(datIn, 0.0, 1.0);
            }
        "#;

        let frag_source = r#"
            #ifdef GL_ES
            precision mediump float;
            #endif
            void main() {
                gl_FragColor = vec4(0.0, 0.0, 0.0, 1.0);
            }
        "#;

        Self::compile(vert_source, frag_source)
    }

    fn update_media(&mut self, new_media_type: MediaType) -> Result<()> {
        eprintln!("Updating media to: {:?}", new_media_type);

        if let Some(texture) = self.media_texture {
            unsafe {
                gl::DeleteTextures(1, &texture);
            }
        }
        self.video_decoder = None;

        let (shader_program, media_texture, video_decoder) = match &new_media_type {
            MediaType::Shader(shader_path) => {
                let program = Self::create_pure_shader(shader_path)?;
                (program, None, None)
            }
            MediaType::Image { path, shader } => {
                let texture = ImageLoader::load_texture(path)?;
                let program = if let Some(shader_path) = shader {
                    Self::create_media_shader(shader_path)?
                } else {
                    Self::create_default_shader()?
                };
                (program, Some(texture), None)
            }
            MediaType::Video { path, shader } => {
                let decoder = VideoDecoder::new(path)?;
                let texture = decoder.texture();
                let program = if let Some(shader_path) = shader {
                    Self::create_media_shader(shader_path)?
                } else {
                    Self::create_default_shader()?
                };
                (program, Some(texture), Some(decoder))
            }
        };

        unsafe {
            gl::DeleteProgram(self.shader_program);
        }

        self.shader_program = shader_program;
        self.media_texture = media_texture;
        self.video_decoder = video_decoder;
        self.media_type = new_media_type;

        Ok(())
    }

    fn create_pure_shader(shader_path: &str) -> Result<u32> {
        let frag_source = load_shader(shader_path)?;

        let vert_source = r#"
            #version 100
            attribute highp vec2 datIn;
            attribute highp vec2 texIn;
            varying vec2 texCoords;
            void main() {
                texCoords = texIn;
                gl_Position = vec4(datIn, 0.0, 1.0);
            }
        "#;

        Self::compile(vert_source, &frag_source)
    }

    fn create_media_shader(shader_path: &str) -> Result<u32> {
        let frag_source = load_shader(shader_path)?;
        let vert_source = vertex_shader();
        Self::compile(vert_source, &frag_source)
    }

    fn create_default_shader() -> Result<u32> {
        let vert_source = vertex_shader();
        let frag_source = default_shader();
        Self::compile(vert_source, frag_source)
    }

    fn compile(vert_source: &str, frag_source: &str) -> Result<u32> {
        unsafe {
            let program = gl::CreateProgram();

            let vert_shader = gl::CreateShader(gl::VERTEX_SHADER);
            let vert_c_str = CString::new(vert_source)?;
            gl::ShaderSource(vert_shader, 1, &vert_c_str.as_ptr(), std::ptr::null());
            gl::CompileShader(vert_shader);
            Self::check_compile(vert_shader, "vertex")?;

            // Create fragment shader
            let frag_shader = gl::CreateShader(gl::FRAGMENT_SHADER);
            let frag_c_str = CString::new(frag_source)?;
            gl::ShaderSource(frag_shader, 1, &frag_c_str.as_ptr(), std::ptr::null());
            gl::CompileShader(frag_shader);
            Self::check_compile(frag_shader, "fragment")?;

            // Link program
            gl::AttachShader(program, vert_shader);
            gl::AttachShader(program, frag_shader);
            gl::LinkProgram(program);
            Self::check_linked(program)?;

            gl::DeleteShader(vert_shader);
            gl::DeleteShader(frag_shader);

            Ok(program)
        }
    }

    fn check_compile(shader: u32, shader_type: &str) -> Result<()> {
        unsafe {
            let mut status = 0;
            gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut status);
            if status == gl::FALSE as i32 {
                let mut log_length = 0;
                gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut log_length);
                let mut log = vec![0u8; log_length as usize];
                gl::GetShaderInfoLog(
                    shader,
                    log_length,
                    std::ptr::null_mut(),
                    log.as_mut_ptr() as *mut i8,
                );
                let log_str = CStr::from_ptr(log.as_ptr() as *const i8).to_string_lossy();
                return Err(anyhow!(
                    "{} shader compilation failed: {}",
                    shader_type,
                    log_str
                ));
            }
        }
        Ok(())
    }

    fn check_linked(program: u32) -> Result<()> {
        unsafe {
            let mut status = 0;
            gl::GetProgramiv(program, gl::LINK_STATUS, &mut status);
            if status == gl::FALSE as i32 {
                let mut log_length = 0;
                gl::GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut log_length);
                let mut log = vec![0u8; log_length as usize];
                gl::GetProgramInfoLog(
                    program,
                    log_length,
                    std::ptr::null_mut(),
                    log.as_mut_ptr() as *mut i8,
                );
                let log_str = CStr::from_ptr(log.as_ptr() as *const i8).to_string_lossy();
                return Err(anyhow!("Program linking failed: {}", log_str));
            }
        }
        Ok(())
    }

    fn setup_geometry() -> Result<(u32, u32)> {
        let vertices: [f32; 16] = [
            -1.0, 1.0, 0.0, 1.0, // Top left
            -1.0, -1.0, 0.0, 0.0, // Bottom left
            1.0, -1.0, 1.0, 0.0, // Bottom right
            1.0, 1.0, 1.0, 1.0, // Top right
        ];

        let indices: [u32; 6] = [0, 1, 2, 2, 3, 0];

        unsafe {
            let mut vbo = 0;
            gl::GenBuffers(1, &mut vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (vertices.len() * std::mem::size_of::<f32>()) as isize,
                vertices.as_ptr() as *const _,
                gl::STATIC_DRAW,
            );

            let mut ebo = 0;
            gl::GenBuffers(1, &mut ebo);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ebo);
            gl::BufferData(
                gl::ELEMENT_ARRAY_BUFFER,
                (indices.len() * std::mem::size_of::<u32>()) as isize,
                indices.as_ptr() as *const _,
                gl::STATIC_DRAW,
            );

            // (location 0)
            gl::VertexAttribPointer(
                0,
                2,
                gl::FLOAT,
                gl::FALSE,
                4 * std::mem::size_of::<f32>() as i32,
                std::ptr::null(),
            );
            gl::EnableVertexAttribArray(0);

            // (location 1)
            gl::VertexAttribPointer(
                1,
                2,
                gl::FLOAT,
                gl::FALSE,
                4 * std::mem::size_of::<f32>() as i32,
                (2 * std::mem::size_of::<f32>()) as *const _,
            );
            gl::EnableVertexAttribArray(1);

            Ok((vbo, ebo))
        }
    }

    fn draw(
        &mut self,
        fifo_reader: &mut Option<FifoReader>,
        output_width: i32,
        output_height: i32,
    ) -> Result<()> {
        unsafe {
            gl::UseProgram(self.shader_program);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            gl::Viewport(0, 0, output_width, output_height);

            if let Some(ref mut decoder) = self.video_decoder {
                decoder.update_frame()?;
            }

            let time_loc =
                gl::GetUniformLocation(self.shader_program, b"time\0".as_ptr() as *const i8);
            if time_loc != -1 {
                let time = (utils::get_time_millis() - self.start_time) as f32 / 1000.0;
                gl::Uniform1f(time_loc, time);
            }

            let resolution_loc =
                gl::GetUniformLocation(self.shader_program, b"resolution\0".as_ptr() as *const i8);
            if resolution_loc != -1 {
                gl::Uniform2f(resolution_loc, output_width as f32, output_height as f32);
            }

            if let Some(texture) = self.media_texture {
                gl::ActiveTexture(gl::TEXTURE0);
                gl::BindTexture(gl::TEXTURE_2D, texture);

                let media_loc =
                    gl::GetUniformLocation(self.shader_program, b"u_media\0".as_ptr() as *const i8);
                if media_loc != -1 {
                    gl::Uniform1i(media_loc, 0);
                }
            }

            if let Some(reader) = fifo_reader {
                let fifo_loc =
                    gl::GetUniformLocation(self.shader_program, b"fifo\0".as_ptr() as *const i8);
                if fifo_loc != -1 {
                    if let Ok(Some(sample)) = reader.read_sample() {
                        let left_val = if !sample.left.is_empty() {
                            sample.left[0] as f32
                        } else {
                            0.0
                        };
                        let right_val = if !sample.right.is_empty() {
                            sample.right[0] as f32
                        } else {
                            0.0
                        };
                        gl::Uniform2f(fifo_loc, right_val, left_val);
                    }
                }
            }

            gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_INT, std::ptr::null());
        }
        Ok(())
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
                                id: name,
                                output,
                                width: 0,
                                height: 0,
                                name: None,
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
        output_id: &u32,
        _: &Connection,
        _: &QueueHandle<AppState>,
    ) {
        match event {
            wl_output::Event::Mode {
                flags,
                width,
                height,
                refresh: _,
            } => {
                if let Ok(mode_flags) = flags.into_result() {
                    if mode_flags.contains(wl_output::Mode::Current) {
                        if let Some(output_info) = state.outputs.get_mut(output_id) {
                            output_info.width = width;
                            output_info.height = height;
                        }
                    }
                }
            }
            _ => {}
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
                    output_info.name = Some(name.clone());
                    if state.monitor_name.is_empty() || name == state.monitor_name {
                        state.target_output = Some(OutputInfo {
                            id: output_info.id,
                            output: output_info.output.clone(),
                            width: output_info.width,
                            height: output_info.height,
                            name: Some(name),
                        });
                    }
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
                state.configured = true;
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

pub fn init(
    monitor: &str,
    media_type: MediaType,
    fps: u16,
    layer_name: Option<&str>,
    _width: u16,
    _height: u16,
    fifo_path: Option<&str>,
    ipc_receiver: Receiver<MediaType>,
) -> Result<()> {
    let conn = Connection::connect_to_env()?;
    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();

    let mut app_state = AppState::new(monitor.to_string());
    let _registry = conn.display().get_registry(&qh, ());

    event_queue.roundtrip(&mut app_state)?;

    if let Some(ref output_manager) = app_state.output_manager {
        for (id, output_info) in &app_state.outputs {
            let _xdg_output = output_manager.get_xdg_output(&output_info.output, &qh, *id);
        }
    }

    event_queue.roundtrip(&mut app_state)?;

    let target_output = if monitor.is_empty() {
        app_state.target_output.take().or_else(|| {
            app_state.outputs.values().next().map(|o| OutputInfo {
                id: o.id,
                output: o.output.clone(),
                width: o.width,
                height: o.height,
                name: o.name.clone(),
            })
        })
    } else {
        app_state.target_output.take()
    }
    .ok_or_else(|| {
        let available: Vec<String> = app_state
            .outputs
            .values()
            .filter_map(|o| o.name.as_ref())
            .cloned()
            .collect();
        anyhow!(
            "Could not find output '{}' (available: {})",
            monitor,
            available.join(", ")
        )
    })?;

    info!(
        "Using output: {} ({}x{})",
        target_output.name.as_deref().unwrap_or("unknown"),
        target_output.width,
        target_output.height
    );

    let compositor = app_state
        .compositor
        .as_ref()
        .ok_or_else(|| anyhow!("Compositor not available"))?;

    let layer_shell = app_state
        .layer_shell
        .as_ref()
        .ok_or_else(|| anyhow!("Layer shell not available"))?;

    let surface = compositor.create_surface(&qh, ());

    let input_region = compositor.create_region(&qh, ());
    let render_region = compositor.create_region(&qh, ());
    render_region.add(0, 0, target_output.width, target_output.height);
    surface.set_opaque_region(Some(&render_region));
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
        Some(&target_output.output),
        layer,
        "papyrust-daemon".to_string(),
        &qh,
        (),
    );

    layer_surface.set_exclusive_zone(-1);
    layer_surface.set_size(target_output.width as u32, target_output.height as u32);
    surface.commit();

    event_queue.roundtrip(&mut app_state)?;

    while !app_state.configured {
        event_queue.blocking_dispatch(&mut app_state)?;
    }

    event_queue.roundtrip(&mut app_state)?;

    // Setup EGL
    let display_ptr = conn.display().id().as_ptr();
    let egl_instance = egl::Instance::new(egl::Static);
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
        wayland_egl::WlEglSurface::new(surface.id(), target_output.width, target_output.height)?;

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

    if fps == 0 {
        egl_instance.swap_interval(egl_display, 1)?;
    } else {
        egl_instance.swap_interval(egl_display, 0)?;
    }

    let mut renderer = MediaRenderer::new(media_type)?;

    let mut fifo_reader = if let Some(path) = fifo_path {
        Some(FifoReader::new(path)?)
    } else {
        None
    };

    info!("Starting render loop");

    loop {
        let frame_start = utils::get_time_millis();

        if let Ok(new_media_type) = ipc_receiver.try_recv() {
            if let Err(e) = renderer.update_media(new_media_type) {
                eprintln!("Failed to update media: {}", e);
            }
        }

        event_queue.dispatch_pending(&mut app_state)?;

        renderer.draw(&mut fifo_reader, target_output.width, target_output.height)?;
        egl_instance.swap_buffers(egl_display, egl_surface)?;

        if fps > 0 {
            let frame_time = utils::get_time_millis() - frame_start;
            let target_frame_time = 1000 / fps as u64;
            if frame_time < target_frame_time {
                utils::sleep_millis(target_frame_time - frame_time);
            }
        }
    }
}

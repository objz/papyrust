use anyhow::{anyhow, Result};
use khronos_egl as egl;
use log::{debug, info};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::unix::io::{AsRawFd, RawFd};
use std::process::Child;
use std::sync::mpsc::Receiver;
use wayland_client::protocol::{wl_compositor, wl_output, wl_region, wl_registry, wl_surface};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_client::backend::ObjectId;
use wayland_protocols::xdg::xdg_output::zv1::client::{zxdg_output_manager_v1, zxdg_output_v1};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use crate::gl_bindings as gl;
use crate::ipc::MediaChange;
use crate::media::{
    default_shader, load_shader, vertex_shader, ImageLoader, MediaType, VideoDecoder,
};
use crate::utils;

const N_SAMPLES: usize = 44100 / 25;

#[derive(Debug, Clone)]
struct OutputInfo {
    _id: u32,
    output: wl_output::WlOutput,
    width: i32,
    height: i32,
    name: Option<String>,
    transform: wl_output::Transform,
    scale: i32,
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
    configured_count: usize,
    total_surfaces: usize,
    pending_configures: HashMap<ObjectId, (u32, u32)>,
}

impl AppState {
    fn new() -> Self {
        Self {
            outputs: HashMap::new(),
            compositor: None,
            layer_shell: None,
            output_manager: None,
            configured_count: 0,
            total_surfaces: 0,
            pending_configures: HashMap::new(),
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
    media_width: u32,
    media_height: u32,
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

        let (shader_program, media_texture, video_decoder, media_width, media_height) =
            if media_type == MediaType::Shader("default".to_string()) {
                let program = Self::default_shader()?;
                (program, None, None, 0, 0)
            } else {
                match &media_type {
                    MediaType::Shader(path) => {
                        let program = Self::create_pure_shader(path)?;
                        (program, None, None, 0, 0)
                    }
                    MediaType::Image { path, shader } => {
                        let img = image::open(path)
                            .map_err(|e| anyhow!("Failed to open image {}: {}", path, e))?;
                        let (w, h) = (img.width(), img.height());
                        let texture = ImageLoader::load_texture(path)?;
                        let program = if let Some(s) = shader {
                            Self::create_media_shader(s)?
                        } else {
                            Self::create_default_shader()?
                        };
                        (program, Some(texture), None, w, h)
                    }
                    MediaType::Video { path, shader } => {
                        let decoder = VideoDecoder::new(path)?;
                        let (w, h) = (decoder.width(), decoder.height());
                        let texture = decoder.texture();
                        let program = if let Some(s) = shader {
                            Self::create_media_shader(s)?
                        } else {
                            Self::create_default_shader()?
                        };
                        (program, Some(texture), Some(decoder), w, h)
                    }
                }
            };

        let (vbo, ebo) = Self::setup_geometry()?;

        Ok(Self {
            shader_program,
            media_texture,
            video_decoder,
            media_width,
            media_height,
            _vbo: vbo,
            _ebo: ebo,
            start_time,
            media_type,
        })
    }
    fn transform(&self, transform: wl_output::Transform, ow: i32, oh: i32) {
        let sw = ow as f32;
        let sh = oh as f32;
        let iw = self.media_width as f32;
        let ih = self.media_height as f32;
        if iw <= 0.0 || ih <= 0.0 {
            return; 
        }

        let scale_factor = f32::max(sw / iw, sh / ih);

        let ndc_w = (iw * scale_factor) / sw;
        let ndc_h = (ih * scale_factor) / sh;

        let (sx, sy) = (ndc_w, ndc_h);

        // quad vertices
        let verts: [f32; 16] = [
            -sx, sy, 0.0, 1.0, -sx, -sy, 0.0, 0.0, sx, -sy, 1.0, 0.0, sx, sy, 1.0, 1.0,
        ];

        unsafe {
            gl::BindBuffer(gl::ARRAY_BUFFER, self._vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (verts.len() * std::mem::size_of::<f32>()) as isize,
                verts.as_ptr() as *const _,
                gl::STATIC_DRAW,
            );
        }
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

        let (shader_program, media_texture, video_decoder, media_width, media_height) =
            match &new_media_type {
                MediaType::Shader(path) => {
                    let program = Self::create_pure_shader(path)?;
                    (program, None, None, 0, 0)
                }
                MediaType::Image { path, shader } => {
                    let img = image::open(path)
                        .map_err(|e| anyhow!("Failed to open image {}: {}", path, e))?;
                    let (w, h) = (img.width(), img.height());
                    let texture = ImageLoader::load_texture(path)?;
                    let program = if let Some(s) = shader {
                        Self::create_media_shader(s)?
                    } else {
                        Self::create_default_shader()?
                    };
                    (program, Some(texture), None, w, h)
                }
                MediaType::Video { path, shader } => {
                    let decoder = VideoDecoder::new(path)?;
                    let (w, h) = (decoder.width(), decoder.height());
                    let texture = decoder.texture();
                    let program = if let Some(s) = shader {
                        Self::create_media_shader(s)?
                    } else {
                        Self::create_default_shader()?
                    };
                    (program, Some(texture), Some(decoder), w, h)
                }
            };

        unsafe {
            gl::DeleteProgram(self.shader_program);
        }

        self.shader_program = shader_program;
        self.media_texture = media_texture;
        self.video_decoder = video_decoder;
        self.media_width = media_width;
        self.media_height = media_height;
        self.media_type = new_media_type;

        Ok(())
    }
    fn create_pure_shader(shader_path: &str) -> Result<u32> {
        let raw = load_shader(shader_path)?;
        let mut version_directive: Option<&str> = None;
        let mut body_lines = Vec::new();
        for line in raw.lines() {
            let trimmed = line.trim_start();
            if version_directive.is_none() && trimmed.starts_with("#version") {
                version_directive = Some(line);
            } else {
                body_lines.push(line);
            }
        }
        body_lines.retain(|l| {
            let t = l.trim_start();
            !(t.starts_with("precision ") && t.ends_with("float;"))
        });
        let mut frag_source = String::new();
        if let Some(v) = version_directive {
            frag_source.push_str(v);
            frag_source.push('\n');
        }
        frag_source.push_str(
            r#"
            #ifdef GL_ES
              #ifdef GL_FRAGMENT_PRECISION_HIGH
                precision highp float;
              #else
                precision mediump float;
              #endif
            #endif
            "#,
        );
        frag_source.push_str(&body_lines.join("\n"));
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
        let raw = load_shader(shader_path)?;
        let mut version_directive: Option<&str> = None;
        let mut body_lines = Vec::new();
        for line in raw.lines() {
            let trimmed = line.trim_start();
            if version_directive.is_none() && trimmed.starts_with("#version") {
                version_directive = Some(line);
            } else {
                body_lines.push(line);
            }
        }
        body_lines.retain(|l| {
            let t = l.trim_start();
            !(t.starts_with("precision ") && t.ends_with("float;"))
        });
        let mut frag_source = String::new();
        if let Some(v) = version_directive {
            frag_source.push_str(v);
            frag_source.push('\n');
        }
        frag_source.push_str(
            r#"
            #ifdef GL_ES
              #ifdef GL_FRAGMENT_PRECISION_HIGH
                precision highp float;
              #else
                precision mediump float;
              #endif
            #endif
            "#,
        );
        frag_source.push_str(&body_lines.join("\n"));
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

            let frag_shader = gl::CreateShader(gl::FRAGMENT_SHADER);
            let frag_c_str = CString::new(frag_source)?;
            gl::ShaderSource(frag_shader, 1, &frag_c_str.as_ptr(), std::ptr::null());
            gl::CompileShader(frag_shader);
            Self::check_compile(frag_shader, "fragment")?;

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
            -1.0, 1.0, 0.0, 1.0, -1.0, -1.0, 0.0, 0.0, 1.0, -1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0,
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

            gl::VertexAttribPointer(
                0,
                2,
                gl::FLOAT,
                gl::FALSE,
                4 * std::mem::size_of::<f32>() as i32,
                std::ptr::null(),
            );
            gl::EnableVertexAttribArray(0);

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
        transform: wl_output::Transform,
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
            self.transform(transform, output_width, output_height);
            gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_INT, std::ptr::null());
        }
        Ok(())
    }
}

struct MonitorState {
    egl_display: egl::Display,
    egl_surface: egl::Surface,
    egl_context: egl::Context,
    renderer: MediaRenderer,
    output_info: OutputInfo,
    _surface: wl_surface::WlSurface,
    layer_surface: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    egl_surface_wrapper: wayland_egl::WlEglSurface,
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
                                _id: name,
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
                width,
                height,
            } => {
                surface.ack_configure(serial);
                state.pending_configures.insert(surface.id(), (width, height));
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

fn create_monitor_state(
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

    surface.set_buffer_transform(wl_output::Transform::Normal);
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
    layer_surface.set_anchor(zwlr_layer_surface_v1::Anchor::Top | zwlr_layer_surface_v1::Anchor::Bottom | zwlr_layer_surface_v1::Anchor::Left | zwlr_layer_surface_v1::Anchor::Right);
    layer_surface.set_size(0, 0);
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
        _surface: surface,
        layer_surface,
        egl_surface_wrapper: egl_surface_wrapper,
    })
}

pub fn init(
    media_type: MediaType,
    fps: u16,
    layer_name: Option<&str>,
    _width: u16,
    _height: u16,
    fifo_path: Option<&str>,
    ipc_receiver: Receiver<MediaChange>,
    mute: bool,
) -> Result<()> {
    let conn = Connection::connect_to_env()?;
    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();
    let mut app_state = AppState::new();
    let _registry = conn.display().get_registry(&qh, ());
    event_queue.roundtrip(&mut app_state)?;

    if let Some(ref om) = app_state.output_manager {
        for (id, info) in &app_state.outputs {
            om.get_xdg_output(&info.output, &qh, *id);
        }
    }
    event_queue.roundtrip(&mut app_state)?;

    let compositor = app_state
        .compositor
        .as_ref()
        .ok_or_else(|| anyhow!("Compositor not available"))?;
    let layer_shell = app_state
        .layer_shell
        .as_ref()
        .ok_or_else(|| anyhow!("Layer shell not available"))?;
    let egl_instance = egl::Instance::new(egl::Static);
    let mut monitor_states = HashMap::new();

    for output_info in app_state.outputs.values() {
        if let Some(name) = &output_info.name {
            let ms = create_monitor_state(
                output_info,
                compositor,
                layer_shell,
                layer_name,
                media_type.clone(),
                &egl_instance,
                &conn,
                &qh,
            )?;
            monitor_states.insert(name.clone(), ms);
            app_state.total_surfaces += 1;
        }
    }

    event_queue.roundtrip(&mut app_state)?;
    while app_state.configured_count < app_state.total_surfaces {
        event_queue.blocking_dispatch(&mut app_state)?;
    }
    event_queue.roundtrip(&mut app_state)?;

    for ms in monitor_states.values() {
        egl_instance.swap_interval(ms.egl_display, if fps == 0 { 1 } else { 0 })?;
    }

    let mut fifo_reader = fifo_path.map(FifoReader::new).transpose()?;
    info!(
        "Starting render loop with {} monitors",
        monitor_states.len()
    );

    let mut last_audio_path: Option<String> = None;
    let mut last_audio_child: Option<Child> = None;

    loop {
        let frame_start = utils::get_time_millis();

        if let Ok(media_change) = ipc_receiver.try_recv() {
            if let MediaType::Video { path, .. } = &media_change.media_type {
                let effective_mute = mute || media_change.mute;

                if effective_mute || last_audio_path.as_deref() != Some(path.as_str()) {
                    if let Some(mut child) = last_audio_child.take() {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                }

                if !effective_mute && last_audio_path.as_deref() != Some(path.as_str()) {
                    let audio_path = path.clone();
                    if let Ok(child) = std::process::Command::new("ffplay")
                        .args(&[
                            "-nodisp",
                            "-autoexit",
                            "-hide_banner",
                            "-loglevel",
                            "error",
                            "-loop",
                            "0",
                            &audio_path,
                        ])
                        .spawn()
                    {
                        last_audio_child = Some(child);
                        last_audio_path = Some(path.clone());
                    }
                } else if effective_mute {
                    last_audio_path = None;
                }
            } else {
                if let Some(mut child) = last_audio_child.take() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                last_audio_path = None;
            }

            if let Some(target) = &media_change.monitor {
                if let Some(ms) = monitor_states.get_mut(target) {
                    egl_instance.make_current(
                        ms.egl_display,
                        Some(ms.egl_surface),
                        Some(ms.egl_surface),
                        Some(ms.egl_context),
                    )?;
                    ms.renderer.update_media(media_change.media_type)?;
                }
            } else {
                for ms in monitor_states.values_mut() {
                    egl_instance.make_current(
                        ms.egl_display,
                        Some(ms.egl_surface),
                        Some(ms.egl_surface),
                        Some(ms.egl_context),
                    )?;
                    ms.renderer.update_media(media_change.media_type.clone())?;
                }
            }
        }

        event_queue.dispatch_pending(&mut app_state)?;
        
        // Handle pending configures and resize EGL windows
        for ms in monitor_states.values_mut() {
            if let Some((w, h)) = app_state.pending_configures.remove(&ms.layer_surface.id()) {
                let new_w = if w == 0 { ms.output_info.width as u32 } else { w };
                let new_h = if h == 0 { ms.output_info.height as u32 } else { h };
                ms.egl_surface_wrapper.resize(new_w as i32, new_h as i32, 0, 0);
                ms.output_info.width = new_w as i32;
                ms.output_info.height = new_h as i32;
            }
        }
        
        for ms in monitor_states.values_mut() {
            egl_instance.make_current(
                ms.egl_display,
                Some(ms.egl_surface),
                Some(ms.egl_surface),
                Some(ms.egl_context),
            )?;
            ms.renderer.draw(
                &mut fifo_reader,
                ms.output_info.width,
                ms.output_info.height,
                ms.output_info.transform,
            )?;
            egl_instance.swap_buffers(ms.egl_display, ms.egl_surface)?;
        }

        if fps > 0 {
            let elapsed = utils::get_time_millis() - frame_start;
            let target = 1000 / fps as u64;
            if elapsed < target {
                utils::sleep_millis(target - elapsed);
            }
        }
    }
}

use crate::utils;
use crate::wayland::fifo::FifoReader;
use anyhow::{anyhow, Result};
use std::ffi::{CStr, CString};
use wayland_client::protocol::{wl_output};

use crate::gl_bindings as gl;
use crate::media::{
    load_shader, ImageLoader, MediaType, VideoDecoder,
};
use crate::utils::{default_shader, vertex_shader};



pub struct MediaRenderer {
    shader: u32,
    texture: Option<u32>,
    decoder: Option<VideoDecoder>,
    vbo: u32,
    media_width: u32,
    media_height: u32,
    start_time: u64,
    media_type: MediaType,
}

impl MediaRenderer {
    pub fn new(media_type: MediaType) -> Result<Self> {
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

        let (vbo, _) = Self::setup_geometry()?;

        Ok(Self {
            shader: shader_program,
            texture: media_texture,
            decoder: video_decoder,
            media_width,
            media_height,
            vbo,
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

        let (sx, sy) = match transform {
            wl_output::Transform::_90
            | wl_output::Transform::_270
            | wl_output::Transform::Flipped90
            | wl_output::Transform::Flipped270 => (ndc_h, ndc_w),
            _ => (ndc_w, ndc_h),
        };

        // quad vertices
        let verts: [f32; 16] = [
            -sx, sy, 0.0, 1.0, -sx, -sy, 0.0, 0.0, sx, -sy, 1.0, 0.0, sx, sy, 1.0, 1.0,
        ];

        unsafe {
            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
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

    pub fn update_media(&mut self, new_media_type: MediaType) -> Result<()> {
        eprintln!("Updating media to: {:?}", new_media_type);

        if let Some(texture) = self.texture {
            unsafe {
                gl::DeleteTextures(1, &texture);
            }
        }
        self.decoder = None;

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
            gl::DeleteProgram(self.shader);
        }

        self.shader = shader_program;
        self.texture = media_texture;
        self.decoder = video_decoder;
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

    pub fn draw(
        &mut self,
        fifo_reader: &mut Option<FifoReader>,
        output_width: i32,
        output_height: i32,
        transform: wl_output::Transform,
    ) -> Result<()> {
        unsafe {
            gl::UseProgram(self.shader);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            gl::Viewport(0, 0, output_width, output_height);

            if let Some(ref mut decoder) = self.decoder {
                decoder.update_frame()?;
            }

            let time_loc =
                gl::GetUniformLocation(self.shader, b"time\0".as_ptr() as *const i8);
            if time_loc != -1 {
                let time = (utils::get_time_millis() - self.start_time) as f32 / 1000.0;
                gl::Uniform1f(time_loc, time);
            }

            let resolution_loc =
                gl::GetUniformLocation(self.shader, b"resolution\0".as_ptr() as *const i8);
            if resolution_loc != -1 {
                gl::Uniform2f(resolution_loc, output_width as f32, output_height as f32);
            }

            if let Some(texture) = self.texture {
                gl::ActiveTexture(gl::TEXTURE0);
                gl::BindTexture(gl::TEXTURE_2D, texture);

                let media_loc =
                    gl::GetUniformLocation(self.shader, b"u_media\0".as_ptr() as *const i8);
                if media_loc != -1 {
                    gl::Uniform1i(media_loc, 0);
                }
            }

            if let Some(reader) = fifo_reader {
                let fifo_loc =
                    gl::GetUniformLocation(self.shader, b"fifo\0".as_ptr() as *const i8);
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

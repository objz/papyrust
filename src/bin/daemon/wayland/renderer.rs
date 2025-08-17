use crate::utils;
use crate::wayland::fifo::FifoReader;
use anyhow::Result;
use std::ffi::CString;
use wayland_client::protocol::wl_output;

use crate::gl_bindings as gl;
use crate::media::{MediaType, ShaderHandler, ImageHandler, VideoHandler, MediaHandler};

// Rename the enum to avoid conflict with the MediaHandler trait
pub enum MediaObject {
    Shader(ShaderHandler),
    Image(ImageHandler),
    Video(VideoHandler),
}

impl MediaObject {
    fn get_texture(&self) -> Option<u32> {
        match self {
            MediaObject::Shader(h) => h.get_texture(),
            MediaObject::Image(h) => h.get_texture(),
            MediaObject::Video(h) => h.get_texture(),
        }
    }

    fn get_dimensions(&self) -> (u32, u32) {
        match self {
            MediaObject::Shader(h) => h.get_dimensions(),
            MediaObject::Image(h) => h.get_dimensions(),
            MediaObject::Video(h) => h.get_dimensions(),
        }
    }

    fn update(&mut self) -> Result<bool> {
        match self {
            MediaObject::Shader(h) => h.update(),
            MediaObject::Image(h) => h.update(),
            MediaObject::Video(h) => h.update(),
        }
    }

    fn has_new_frame(&self) -> bool {
        match self {
            MediaObject::Shader(h) => h.has_new_frame(),
            MediaObject::Image(h) => h.has_new_frame(),
            MediaObject::Video(h) => h.has_new_frame(),
        }
    }

    fn get_shader_program(&self) -> u32 {
        match self {
            MediaObject::Shader(h) => h.get_shader_program(),
            MediaObject::Image(h) => h.get_shader_program(),
            MediaObject::Video(h) => h.get_shader_program(),
        }
    }
}

pub struct MediaRenderer {
    media_object: MediaObject,
    vbo: u32,
    ebo: u32,
    start_time: u64,
}

impl MediaRenderer {
    pub fn new(media_type: MediaType, fps: u16) -> Result<Self> {
        tracing::info!(
            event = "renderer_create",
            ?media_type,
            fps,
            "Creating MediaRenderer"
        );

        let start_time = utils::get_time_millis();

        Self::initialize_gl()?;

        let media_object = Self::create_media_object(media_type, fps)?;
        let (vbo, ebo) = Self::setup_geometry()?;

        tracing::debug!(
            event = "renderer_ready",
            has_texture = media_object.get_texture().is_some(),
            "Renderer initialized"
        );

        Ok(Self {
            media_object,
            vbo,
            ebo,
            start_time,
        })
    }

    fn initialize_gl() -> Result<()> {
        unsafe {
            gl::load_with(|s| {
                let c_str = CString::new(s).unwrap();
                let proc_addr = match std::ffi::CStr::from_bytes_with_nul(b"eglGetProcAddress\0") {
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
        Ok(())
    }

    fn create_media_object(media_type: MediaType, fps: u16) -> Result<MediaObject> {
        match media_type {
            MediaType::Shader(path) => {
                let shader_path = if path == "default" { None } else { Some(path.as_str()) };
                Ok(MediaObject::Shader(ShaderHandler::new(shader_path)?))
            }
            MediaType::Image { path, shader } => {
                Ok(MediaObject::Image(ImageHandler::new(&path, shader.as_deref())?))
            }
            MediaType::Video { path, shader } => {
                let forced_fps = if fps > 0 { Some(fps as f64) } else { None };
                Ok(MediaObject::Video(VideoHandler::new(&path, shader.as_deref(), forced_fps)?))
            }
        }
    }

    pub fn has_new_frame(&self) -> bool {
        self.media_object.has_new_frame()
    }

    fn update_geometry(&self, output_width: i32, output_height: i32) {
        let output_w = output_width as f32;
        let output_h = output_height as f32;
        let (media_width, media_height) = self.media_object.get_dimensions();
        let media_w = media_width as f32;
        let media_h = media_height as f32;

        if media_w <= 0.0 || media_h <= 0.0 {
            let verts: [f32; 16] = [
                -1.0, 1.0, 0.0, 0.0, -1.0, -1.0, 0.0, 1.0, 1.0, -1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0,
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
            return;
        }

        let media_aspect = media_w / media_h;
        let output_aspect = output_w / output_h;

        let (scale_x, scale_y) = if media_aspect > output_aspect {
            let scale = output_h / media_h;
            let scaled_width = media_w * scale;
            let overflow = (scaled_width - output_w) / output_w;
            (1.0 + overflow, 1.0)
        } else {
            let scale = output_w / media_w;
            let scaled_height = media_h * scale;
            let overflow = (scaled_height - output_h) / output_h;
            (1.0, 1.0 + overflow)
        };

        let u_min = (1.0 - 1.0 / scale_x) * 0.5;
        let u_max = 1.0 - u_min;
        let v_min = (1.0 - 1.0 / scale_y) * 0.5;
        let v_max = 1.0 - v_min;

        let verts: [f32; 16] = [
            -1.0, 1.0, u_min, v_min, -1.0, -1.0, u_min, v_max, 1.0, -1.0, u_max, v_max, 1.0, 1.0,
            u_max, v_min,
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

    pub fn update_media(&mut self, new_media_type: MediaType, fps: u16) -> Result<()> {
        tracing::info!(
            event = "renderer_media_update",
            ?new_media_type,
            fps,
            "Updating renderer media"
        );

        self.media_object = Self::create_media_object(new_media_type, fps)?;

        tracing::debug!(
            event = "renderer_media_ready",
            dimensions = ?self.media_object.get_dimensions(),
            has_texture = self.media_object.get_texture().is_some(),
            "Media update complete"
        );
        Ok(())
    }

    fn setup_geometry() -> Result<(u32, u32)> {
        let vertices: [f32; 16] = [
            -1.0, 1.0, 0.0, 0.0, -1.0, -1.0, 0.0, 1.0, 1.0, -1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0,
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
        _transform: wl_output::Transform,
    ) -> Result<()> {
        unsafe {
            let shader_program = self.media_object.get_shader_program();
            gl::UseProgram(shader_program);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            gl::Viewport(0, 0, output_width, output_height);

            // Update media (videos need frame updates)
            let _ = self.media_object.update()?;

            // Set shader uniforms
            self.set_uniforms(shader_program, output_width, output_height, fifo_reader)?;

            // Bind texture if available
            if let Some(texture) = self.media_object.get_texture() {
                gl::ActiveTexture(gl::TEXTURE0);
                gl::BindTexture(gl::TEXTURE_2D, texture);

                let media_loc = gl::GetUniformLocation(shader_program, b"u_media\0".as_ptr() as *const i8);
                if media_loc != -1 {
                    gl::Uniform1i(media_loc, 0);
                }
            }

            // Update geometry and draw
            self.update_geometry(output_width, output_height);
            gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_INT, std::ptr::null());
        }
        Ok(())
    }

    fn set_uniforms(
        &self,
        shader_program: u32,
        output_width: i32,
        output_height: i32,
        fifo_reader: &mut Option<FifoReader>,
    ) -> Result<()> {
        unsafe {
            // Time uniform
            let time_loc = gl::GetUniformLocation(shader_program, b"time\0".as_ptr() as *const i8);
            if time_loc != -1 {
                let time = (utils::get_time_millis() - self.start_time) as f32 / 1000.0;
                gl::Uniform1f(time_loc, time);
            }

            // Resolution uniform
            let resolution_loc = gl::GetUniformLocation(shader_program, b"resolution\0".as_ptr() as *const i8);
            if resolution_loc != -1 {
                gl::Uniform2f(resolution_loc, output_width as f32, output_height as f32);
            }

            // FIFO uniform for audio data
            if let Some(reader) = fifo_reader {
                let fifo_loc = gl::GetUniformLocation(shader_program, b"fifo\0".as_ptr() as *const i8);
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
        }
        Ok(())
    }
}

impl Drop for MediaRenderer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &self.vbo);
            gl::DeleteBuffers(1, &self.ebo);
        }
    }
}

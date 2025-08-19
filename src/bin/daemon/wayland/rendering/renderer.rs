use crate::gl_bindings as gl;
use crate::media::{ImageHandler, MediaHandler, MediaType, ShaderHandler, VideoHandler};
use crate::utils;
use crate::wayland::types::RenderContext;
use anyhow::Result;
use std::ffi::CString;

pub enum MediaObject {
    Shader(ShaderHandler),
    Image(ImageHandler),
    Video(VideoHandler),
}

impl MediaObject {
    fn as_handler(&self) -> &dyn MediaHandler {
        match self {
            MediaObject::Shader(h) => h,
            MediaObject::Image(h) => h,
            MediaObject::Video(h) => h,
        }
    }

    fn as_handler_mut(&mut self) -> &mut dyn MediaHandler {
        match self {
            MediaObject::Shader(h) => h,
            MediaObject::Image(h) => h,
            MediaObject::Video(h) => h,
        }
    }

    // Add method to get video handler specifically
    fn as_video_handler_mut(&mut self) -> Option<&mut VideoHandler> {
        match self {
            MediaObject::Video(h) => Some(h),
            _ => None,
        }
    }
}

pub struct MediaRenderer {
    current_media: Option<MediaObject>,
    loading_media: Option<MediaObject>,
    pending_media_type: Option<(MediaType, u16)>,
    vbo: u32,
    ebo: u32,
    vao: u32,
    start_time: u64,
    loading_in_background: bool,
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
        let (vbo, ebo, vao) = Self::setup_geometry()?;

        let mut renderer = Self {
            current_media: None,
            loading_media: None,
            pending_media_type: Some((media_type, fps)),
            vbo,
            ebo,
            vao,
            start_time,
            loading_in_background: false,
        };

        renderer.ensure_resources()?;

        Ok(renderer)
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
                let shader_path = if path == "default" {
                    None
                } else {
                    Some(path.as_str())
                };
                Ok(MediaObject::Shader(ShaderHandler::new(shader_path)?))
            }
            MediaType::Image { path, shader } => Ok(MediaObject::Image(ImageHandler::new(
                &path,
                shader.as_deref(),
            )?)),
            MediaType::Video { path, shader } => {
                let forced_fps = if fps > 0 { Some(fps as f64) } else { None };
                Ok(MediaObject::Video(VideoHandler::new(
                    &path,
                    shader.as_deref(),
                    forced_fps,
                )?))
            }
        }
    }

    pub fn has_new_frame(&self) -> bool {
        if let Some(ref media) = self.current_media {
            media.as_handler().has_new_frame()
        } else if let Some(ref loading) = self.loading_media {
            loading.as_handler().has_new_frame()
        } else {
            false
        }
    }

    // Add method to check for video restarts
    pub fn check_video_restart(&mut self) -> bool {
        if let Some(ref mut media) = self.current_media {
            if let Some(video_handler) = media.as_video_handler_mut() {
                return video_handler.take_restart_flag();
            }
        }
        false
    }

    pub fn update_media(&mut self, new_media_type: MediaType, fps: u16) -> Result<()> {
        tracing::info!(
            event = "renderer_media_update",
            ?new_media_type,
            fps,
            loading_in_background = self.loading_in_background,
            "Updating renderer media"
        );

        self.pending_media_type = Some((new_media_type, fps));
        self.loading_in_background = true;

        Ok(())
    }

    fn ensure_resources(&mut self) -> Result<()> {
        if let Some((media_type, fps)) = self.pending_media_type.take() {
            match Self::create_media_object(media_type.clone(), fps) {
                Ok(new_media) => {
                    let is_ready = match &new_media {
                        MediaObject::Shader(_) => true, 
                        MediaObject::Image(img_handler) => {
                            img_handler.get_texture().is_some()
                        }
                        MediaObject::Video(vid_handler) => {
                            let (w, h) = vid_handler.get_dimensions();
                            w > 0 && h > 0
                        }
                    };

                    if is_ready || self.current_media.is_none() {
                        self.current_media = Some(new_media);
                        self.loading_media = None;
                        self.loading_in_background = false;

                        tracing::info!(
                            event = "media_transition_immediate",
                            ?media_type,
                            "New media ready immediately, transitioning"
                        );
                    } else {
                        self.loading_media = Some(new_media);

                        tracing::info!(
                            event = "media_loading_background",
                            ?media_type,
                            "New media loading in background"
                        );
                    }
                }
                Err(e) => {
                    tracing::error!(
                        event = "media_load_error",
                        error = %e,
                        ?media_type,
                        "Failed to load new media"
                    );
                    self.loading_in_background = false;
                    return Err(e);
                }
            }
        }

        if self.loading_in_background && self.loading_media.is_some() {
            let should_transition = if let Some(ref loading_media) = self.loading_media {
                match loading_media {
                    MediaObject::Image(img_handler) => {
                        img_handler.get_texture().is_some()
                    }
                    MediaObject::Video(vid_handler) => {
                        let (w, h) = vid_handler.get_dimensions();
                        w > 0 && h > 0 && vid_handler.get_texture().is_some()
                    }
                    MediaObject::Shader(_) => true, 
                }
            } else {
                false
            };

            if should_transition {
                if let Some(new_media) = self.loading_media.take() {
                    self.current_media = Some(new_media);
                    self.loading_in_background = false;

                    tracing::info!(
                        event = "media_transition_complete",
                        "Background loaded media is now active"
                    );
                }
            }
        }

        Ok(())
    }

    fn setup_geometry() -> Result<(u32, u32, u32)> {
        let vertices: [f32; 16] = [
            -1.0, 1.0, 0.0, 0.0, -1.0, -1.0, 0.0, 1.0, 1.0, -1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0,
        ];

        let indices: [u32; 6] = [0, 1, 2, 2, 3, 0];

        unsafe {
            let mut vao = 0;
            gl::GenVertexArrays(1, &mut vao);
            gl::BindVertexArray(vao);

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

            gl::BindVertexArray(0);

            Ok((vbo, ebo, vao))
        }
    }

    pub fn draw(&mut self, context: &mut RenderContext) -> Result<()> {
        self.ensure_resources()?;

        if let Some(ref mut media) = self.current_media {
            let _ = media.as_handler_mut().update()?;
        }

        if let Some(ref mut loading_media) = self.loading_media {
            let _ = loading_media.as_handler_mut().update()?;
        }

        let media_to_render = self.current_media.as_ref().or(self.loading_media.as_ref());

        let Some(media_object) = media_to_render else {
            unsafe {
                gl::Clear(gl::COLOR_BUFFER_BIT);
            }
            return Ok(());
        };

        let handler = media_object.as_handler();
        let program = handler.get_shader_program();

        unsafe {
            program.use_program();

            gl::Clear(gl::COLOR_BUFFER_BIT);
            gl::Viewport(0, 0, context.width, context.height);

            let time_loc = program.get_uniform_location("time");
            if time_loc != -1 {
                let raw_time = (utils::get_time_millis() - self.start_time) as f32 / 1000.0;
                let time = raw_time % 3600.0;
                gl::Uniform1f(time_loc, time);
            }

            let resolution_loc = program.get_uniform_location("resolution");
            if resolution_loc != -1 {
                gl::Uniform2f(resolution_loc, context.width as f32, context.height as f32);
            }

            if let Some(ref mut reader) = context.fifo_reader {
                let fifo_loc = program.get_uniform_location("fifo");
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

            if let Some(texture) = handler.get_texture() {
                gl::ActiveTexture(gl::TEXTURE0);
                texture.bind();

                let media_loc = program.get_uniform_location("u_media");
                if media_loc != -1 {
                    gl::Uniform1i(media_loc, 0);
                }
            }

            let (media_width, media_height) = handler.get_dimensions();
            self.update_geometry(context.width, context.height, media_width, media_height);

            gl::BindVertexArray(self.vao);
            gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_INT, std::ptr::null());
            gl::BindVertexArray(0);
        }
        Ok(())
    }

    fn update_geometry(
        &self,
        output_width: i32,
        output_height: i32,
        media_width: u32,
        media_height: u32,
    ) {
        let output_w = output_width as f32;
        let output_h = output_height as f32;
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
}

impl Drop for MediaRenderer {
    fn drop(&mut self) {
        unsafe {
            if self.vao != 0 {
                gl::DeleteVertexArrays(1, &self.vao);
            }
            if self.vbo != 0 {
                gl::DeleteBuffers(1, &self.vbo);
            }
            if self.ebo != 0 {
                gl::DeleteBuffers(1, &self.ebo);
            }
        }
    }
}

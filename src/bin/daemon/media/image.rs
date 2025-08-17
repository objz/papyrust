use anyhow::{Result, anyhow};
use image as img_crate;
use crate::gl_bindings as gl;
use crate::media::{MediaHandler, shader::ShaderHandler};
use crate::utils::{vertex_shader, default_shader, compile_shader};

pub struct ImageHandler {
    texture: u32,
    width: u32,
    height: u32,
    shader_program: u32,
}

impl ImageHandler {
    pub fn new(path: &str, shader_path: Option<&str>) -> Result<Self> {
        tracing::info!(
            event = "image_create",
            path = %path,
            shader = shader_path.unwrap_or("default"),
            "Creating image handler"
        );

        let texture = Self::load_texture(path)?;
        let img = img_crate::open(path)
            .map_err(|e| anyhow!("Failed to open image {}: {}", path, e))?;
        let (width, height) = (img.width(), img.height());

        let shader_program = if let Some(shader_path) = shader_path {
            ShaderHandler::create_media_shader(shader_path)?
        } else {
            Self::create_default_shader()?
        };

        tracing::debug!(
            event = "image_loaded",
            width,
            height,
            texture,
            "Image loaded successfully"
        );

        Ok(Self {
            texture,
            width,
            height,
            shader_program,
        })
    }

    pub fn get_shader_program(&self) -> u32 {
        self.shader_program
    }

    fn load_texture(path: &str) -> Result<u32> {
        tracing::info!(event = "texture_load", path = %path, "Loading texture");

        let img = img_crate::open(path)
            .map_err(|e| anyhow!("Failed to load image {}: {}", path, e))?;
        let rgba = img.to_rgba8();
        let (width, height) = (img.width(), img.height());

        tracing::debug!(event = "image_info", width, height, "Image decoded");

        let mut texture = 0;
        unsafe {
            gl::GenTextures(1, &mut texture);
            gl::BindTexture(gl::TEXTURE_2D, texture);

            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 4);

            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);

            gl::TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_MIN_FILTER,
                gl::LINEAR_MIPMAP_LINEAR as i32,
            );
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);

            let mut max_anisotropy = 0.0f32;
            gl::GetFloatv(0x84FE, &mut max_anisotropy);
            if max_anisotropy > 1.0 {
                let anisotropy = max_anisotropy.min(16.0);
                gl::TexParameterf(gl::TEXTURE_2D, 0x84FE, anisotropy);
                tracing::debug!(
                    event = "anisotropic_filtering",
                    anisotropy,
                    "Applied anisotropic filtering"
                );
            }

            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA as i32,
                width as i32,
                height as i32,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                rgba.as_ptr() as *const _,
            );

            gl::GenerateMipmap(gl::TEXTURE_2D);

            tracing::debug!(
                event = "texture_created",
                width,
                height,
                texture,
                "High-quality texture created with mipmaps"
            );
        }

        Ok(texture)
    }

    fn create_default_shader() -> Result<u32> {
        let vert_source = vertex_shader();
        let frag_source = default_shader();
        compile_shader(vert_source, frag_source)
    }
}

impl MediaHandler for ImageHandler {
    fn get_texture(&self) -> Option<u32> {
        Some(self.texture)
    }

    fn get_dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn update(&mut self) -> Result<bool> {
        Ok(false) // Images don't need updates
    }

    fn has_new_frame(&self) -> bool {
        false // Images don't have frames
    }
}

impl Drop for ImageHandler {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(1, &self.texture);
            gl::DeleteProgram(self.shader_program);
        }
    }
}

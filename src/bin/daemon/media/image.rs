use anyhow::{Result, anyhow};
use image as img_crate;
use crate::gl_utils::GlTexture;
use crate::media::{MediaHandler, BaseMediaHandler};

pub struct ImageHandler {
    base: BaseMediaHandler,
}

impl ImageHandler {
    pub fn new(path: &str, shader_path: Option<&str>) -> Result<Self> {
        tracing::info!(
            event = "image_create",
            path = %path,
            shader = shader_path.unwrap_or("default"),
            "Creating image handler"
        );

        let mut base = BaseMediaHandler::new_with_shader(shader_path)?;
        let texture = Self::load_texture(path)?;
        base.dimensions = (texture.width, texture.height);
        base.texture = Some(texture);

        tracing::debug!(
            event = "image_loaded",
            width = base.dimensions.0,
            height = base.dimensions.1,
            "Image loaded successfully"
        );

        Ok(Self { base })
    }

    fn load_texture(path: &str) -> Result<GlTexture> {
        tracing::info!(event = "texture_load", path = %path, "Loading texture");

        let img = img_crate::open(path)
            .map_err(|e| anyhow!("Failed to load image {}: {}", path, e))?;
        let rgba = img.to_rgba8();
        let (width, height) = (img.width(), img.height());

        tracing::debug!(event = "image_info", width, height, "Image decoded");

        GlTexture::from_rgba_data(width, height, &rgba, true)
    }
}

impl MediaHandler for ImageHandler {
    fn get_texture(&self) -> Option<&GlTexture> {
        self.base.texture.as_ref()
    }

    fn get_dimensions(&self) -> (u32, u32) {
        self.base.dimensions
    }

    fn update(&mut self) -> Result<bool> {
        Ok(false)
    }

    fn has_new_frame(&self) -> bool {
        false 
    }

    fn get_shader_program(&self) -> &crate::gl_utils::GlProgram {
        &self.base.shader_program
    }
}

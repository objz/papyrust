use anyhow::Result;
use crate::media::{MediaHandler, BaseMediaHandler};
use crate::gl_utils::GlTexture;

pub struct ShaderHandler {
    base: BaseMediaHandler,
}

impl ShaderHandler {
    pub fn new(path: Option<&str>) -> Result<Self> {
        tracing::info!(
            event = "shader_create",
            path = path.unwrap_or("default"),
            "Creating shader handler"
        );

        let base = BaseMediaHandler::new_pure_shader(path)?;
        Ok(Self { base })
    }
}

impl MediaHandler for ShaderHandler {
    fn get_texture(&self) -> Option<&GlTexture> {
        None
    }

    fn get_dimensions(&self) -> (u32, u32) {
        (0, 0)
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

use crate::gl_utils::{GlProgram, GlTexture};
use crate::utils;
use anyhow::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum MediaType {
    Shader(String),
    Image {
        path: String,
        shader: Option<String>,
    },
    Video {
        path: String,
        shader: Option<String>,
    },
}

pub trait MediaHandler {
    fn get_texture(&self) -> Option<&GlTexture>;
    fn get_dimensions(&self) -> (u32, u32);
    fn update(&mut self) -> Result<bool>;
    fn has_new_frame(&self) -> bool;
    fn get_shader_program(&self) -> &GlProgram;
}

pub struct BaseMediaHandler {
    pub shader_program: GlProgram,
    pub texture: Option<GlTexture>,
    pub dimensions: (u32, u32),
    pub has_new_frame: bool,
}

impl BaseMediaHandler {
    pub fn new_with_shader(shader_path: Option<&str>) -> Result<Self> {
        let shader_program = if let Some(path) = shader_path {
            if path == "default" {
                Self::create_default_shader()?
            } else {
                Self::create_custom_shader(path)?
            }
        } else {
            Self::create_default_shader()?
        };

        Ok(Self {
            shader_program,
            texture: None,
            dimensions: (0, 0),
            has_new_frame: false,
        })
    }

    pub fn new_pure_shader(shader_path: Option<&str>) -> Result<Self> {
        let shader_program = if let Some(path) = shader_path {
            if path == "default" {
                Self::create_default_shader()?
            } else {
                Self::create_pure_shader(path)?
            }
        } else {
            Self::create_default_shader()?
        };

        Ok(Self {
            shader_program,
            texture: None,
            dimensions: (0, 0),
            has_new_frame: false,
        })
    }

    fn create_default_shader() -> Result<GlProgram> {
        let vert_source = utils::vertex_shader();
        let frag_source = utils::default_shader();
        GlProgram::new(vert_source, frag_source)
    }

    fn create_custom_shader(shader_path: &str) -> Result<GlProgram> {
        let raw = Self::load_shader_file(shader_path)?;
        let frag_source = utils::prepare_shader_source(&raw);
        let vert_source = utils::vertex_shader();
        GlProgram::new(vert_source, &frag_source)
    }

    fn create_pure_shader(shader_path: &str) -> Result<GlProgram> {
        let raw = Self::load_shader_file(shader_path)?;
        let frag_source = utils::prepare_shader_source(&raw);

        let vert_source = r#"
            #version 100
            attribute highp vec2 datIn;
            attribute highp vec2 texIn;
            varying highp vec2 texCoords;
            void main() {
                texCoords = texIn;
                gl_Position = vec4(datIn, 0.0, 1.0);
            }
        "#;

        GlProgram::new(vert_source, &frag_source)
    }

    fn load_shader_file(path: &str) -> Result<String> {
        use anyhow::anyhow;
        use std::fs::File;
        use std::io::Read;

        let mut file =
            File::open(path).map_err(|e| anyhow!("Failed to open shader file {}: {}", path, e))?;
        let mut source = String::new();
        file.read_to_string(&mut source)
            .map_err(|e| anyhow!("Failed to read shader file {}: {}", path, e))?;
        Ok(source)
    }
}

pub mod image;
pub mod shader;
pub mod video;

pub use image::ImageHandler;
pub use shader::ShaderHandler;
pub use video::VideoHandler;

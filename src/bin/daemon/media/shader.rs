use anyhow::{anyhow, Result};
use std::fs::File;
use std::io::Read;
use crate::utils::{compile_shader, vertex_shader, default_shader, prepare_shader_source};
use crate::media::MediaHandler;

pub struct ShaderHandler {
    shader_program: u32,
}

impl ShaderHandler {
    pub fn new(path: Option<&str>) -> Result<Self> {
        tracing::info!(
            event = "shader_create",
            path = path.unwrap_or("default"),
            "Creating shader handler"
        );

        let shader_program = if let Some(shader_path) = path {
            if shader_path == "default" {
                Self::create_default_shader()?
            } else {
                Self::create_pure_shader(shader_path)?
            }
        } else {
            Self::create_default_shader()?
        };

        Ok(Self { shader_program })
    }

    pub fn get_shader_program(&self) -> u32 {
        self.shader_program
    }

    fn load_shader(path: &str) -> Result<String> {
        let mut file = File::open(path)
            .map_err(|e| anyhow!("Failed to open shader file {}: {}", path, e))?;
        let mut source = String::new();
        file.read_to_string(&mut source)
            .map_err(|e| anyhow!("Failed to read shader file {}: {}", path, e))?;
        Ok(source)
    }

    fn create_pure_shader(shader_path: &str) -> Result<u32> {
        let raw = Self::load_shader(shader_path)?;
        let frag_source = prepare_shader_source(&raw);
        
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
        
        compile_shader(vert_source, &frag_source)
    }

    fn create_default_shader() -> Result<u32> {
        let vert_source = vertex_shader();
        let frag_source = default_shader();
        compile_shader(vert_source, frag_source)
    }

    pub fn create_media_shader(shader_path: &str) -> Result<u32> {
        let raw = Self::load_shader(shader_path)?;
        let frag_source = prepare_shader_source(&raw);
        let vert_source = vertex_shader();
        compile_shader(vert_source, &frag_source)
    }
}

impl MediaHandler for ShaderHandler {
    fn get_texture(&self) -> Option<u32> {
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
}

impl Drop for ShaderHandler {
    fn drop(&mut self) {
        unsafe {
            crate::gl_bindings::DeleteProgram(self.shader_program);
        }
    }
}

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
    fn get_texture(&self) -> Option<u32>;
    fn get_dimensions(&self) -> (u32, u32);
    fn update(&mut self) -> Result<bool>;
    fn has_new_frame(&self) -> bool;
}

pub mod image;
pub mod shader;
pub mod video;

pub use image::ImageHandler;
pub use shader::ShaderHandler;
pub use video::VideoHandler;

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

pub mod image;
pub mod shader;
pub mod video;

pub use image::load_texture;
pub use shader::load_shader;
pub use video::VideoDecoder;

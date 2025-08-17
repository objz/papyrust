use anyhow::{Result, anyhow};
use image as _;

use crate::gl_bindings as gl;

pub fn load_texture(path: &str) -> Result<u32> {
    tracing::info!(event = "image_load", path = %path, "Loading image");

    let img = image::open(path).map_err(|e| anyhow!("Failed to load image {}: {}", path, e))?;
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


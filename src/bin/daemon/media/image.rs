use anyhow::{anyhow, Result};
use image as _;
use std::sync::{Arc, OnceLock};

use crate::gl_bindings as gl;
use crate::lossless_scaling::{LosslessScaler, ScalingAlgorithm};

pub struct ImageLoader {
    _scaler: Option<Arc<LosslessScaler>>,
}

impl ImageLoader {
    pub fn new() -> Self {
        Self { _scaler: None }
    }

    #[allow(dead_code)]
    pub async fn new_with_scaler(algorithm: ScalingAlgorithm) -> Result<Self> {
        let scaler = LosslessScaler::new(algorithm).await?;
        Ok(Self {
            _scaler: Some(Arc::new(scaler)),
        })
    }

    pub fn load_texture(&self, path: &str) -> Result<u32> {
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

    pub fn load_texture_scaled(
        &self,
        path: &str,
        target_width: u32,
        target_height: u32,
        sharpening: f32,
    ) -> Result<u32> {
        if let Some(ref scaler) = self._scaler {
            tracing::info!(
                event = "image_load_scaled",
                path = %path,
                target_width,
                target_height,
                sharpening,
                "Loading and scaling image with lossless algorithm"
            );

            let img =
                image::open(path).map_err(|e| anyhow!("Failed to load image {}: {}", path, e))?;
            let rgba = img.to_rgba8();
            let (orig_width, orig_height) = (img.width(), img.height());

            let final_data = if orig_width != target_width || orig_height != target_height {
                let scaled_data = scaler.scale_texture(
                    &rgba,
                    orig_width,
                    orig_height,
                    target_width,
                    target_height,
                    sharpening,
                )?;
                scaled_data
            } else {
                rgba.into_raw()
            };

            let mut texture = 0;
            unsafe {
                gl::GenTextures(1, &mut texture);
                gl::BindTexture(gl::TEXTURE_2D, texture);
                gl::PixelStorei(gl::UNPACK_ALIGNMENT, 4);

                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);

                gl::TexImage2D(
                    gl::TEXTURE_2D,
                    0,
                    gl::RGBA as i32,
                    target_width as i32,
                    target_height as i32,
                    0,
                    gl::RGBA,
                    gl::UNSIGNED_BYTE,
                    final_data.as_ptr() as *const _,
                );
            }

            tracing::debug!(
                event = "scaled_texture_created",
                orig_width,
                orig_height,
                target_width,
                target_height,
                texture,
                "Lossless scaled texture created"
            );

            Ok(texture)
        } else {
            self.load_texture(path)
        }
    }
}

static IMAGE_LOADER: OnceLock<ImageLoader> = OnceLock::new();

fn get_image_loader() -> &'static ImageLoader {
    IMAGE_LOADER.get_or_init(|| ImageLoader::new())
}

pub fn load_texture(path: &str) -> Result<u32> {
    get_image_loader().load_texture(path)
}

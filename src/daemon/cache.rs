use anyhow::{anyhow, Result};
use fast_image_resize::IntoImageView;
use fast_image_resize::IntoImageViewMut;
use image::{codecs::gif::GifDecoder, AnimationDecoder, Delay, RgbaImage};
use log::{debug, info};
use std::collections::HashMap;
use std::io::Cursor;
use std::time::Duration;

use crate::config::ScalingMode;

#[derive(Debug, Clone)]
pub struct GifFrame {
    pub data: Vec<u8>,
    pub delay: Duration,
    pub compressed: bool,
}

pub struct GifCache {
    cache: HashMap<String, Vec<GifFrame>>,
    max_cache_size: usize,
    compression_enabled: bool,
}

impl GifCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            max_cache_size: 100 * 1024 * 1024, // 100MB default
            compression_enabled: true,
        }
    }

    pub fn new_with_config(max_size: usize, enable_compression: bool) -> Self {
        Self {
            cache: HashMap::new(),
            max_cache_size: max_size,
            compression_enabled: enable_compression,
        }
    }

    pub async fn get_or_load_frames(
        &mut self,
        path: &str,
        target_width: u32,
        target_height: u32,
        scaling_mode: ScalingMode,
    ) -> Result<Vec<GifFrame>> {
        let cache_key = format!(
            "{}:{}x{}:{:?}",
            path, target_width, target_height, scaling_mode
        );

        if let Some(frames) = self.cache.get(&cache_key) {
            debug!("Using cached GIF frames for: {}", path);
            return Ok(frames.clone());
        }

        info!("Loading and processing GIF: {}", path);
        let frames = self
            .load_and_process_gif(path, target_width, target_height, scaling_mode)
            .await?;

        // Check cache size before adding
        if self.get_cache_size() + self.estimate_frames_size(&frames) > self.max_cache_size {
            self.evict_oldest();
        }

        self.cache.insert(cache_key, frames.clone());
        info!("Cached {} frames for GIF: {}", frames.len(), path);

        Ok(frames)
    }

    async fn load_and_process_gif(
        &self,
        path: &str,
        target_width: u32,
        target_height: u32,
        scaling_mode: ScalingMode,
    ) -> Result<Vec<GifFrame>> {
        let gif_data = tokio::fs::read(path).await?;
        let cursor = Cursor::new(gif_data);

        let decoder = GifDecoder::new(cursor)?;
        let frames_iter = decoder.into_frames();

        let mut processed_frames = Vec::new();

        for (frame_index, frame_result) in frames_iter.enumerate() {
            let frame = frame_result?;
            let delay = Self::frame_delay_to_duration(frame.delay());

            // Convert frame to RGBA
            let rgba_image = frame.into_buffer();

            // Process the frame (resize, scale, etc.)
            let processed_image =
                self.process_gif_frame(rgba_image, target_width, target_height, scaling_mode)?;

            // Convert to raw bytes
            let raw_data = processed_image.into_raw();

            // Optionally compress the frame data
            let frame_data = if self.compression_enabled {
                self.compress_frame_data(&raw_data)?
            } else {
                raw_data
            };

            let gif_frame = GifFrame {
                data: frame_data,
                delay,
                compressed: self.compression_enabled,
            };

            processed_frames.push(gif_frame);

            if frame_index % 10 == 0 {
                debug!("Processed frame {} of GIF: {}", frame_index + 1, path);
            }
        }

        if processed_frames.is_empty() {
            return Err(anyhow!("No frames found in GIF: {}", path));
        }

        info!(
            "Successfully processed {} frames from GIF: {}",
            processed_frames.len(),
            path
        );
        Ok(processed_frames)
    }

    fn process_gif_frame(
        &self,
        image: RgbaImage,
        target_width: u32,
        target_height: u32,
        scaling_mode: ScalingMode,
    ) -> Result<RgbaImage> {
        let (img_width, img_height) = image.dimensions();

        // If already the right size, return as-is
        if img_width == target_width && img_height == target_height {
            return Ok(image);
        }

        match scaling_mode {
            ScalingMode::Fill => {
                // Scale to fill, crop excess
                let scale_x = target_width as f64 / img_width as f64;
                let scale_y = target_height as f64 / img_height as f64;
                let scale = scale_x.max(scale_y);

                let new_width = (img_width as f64 * scale) as u32;
                let new_height = (img_height as f64 * scale) as u32;

                let scaled = self.resize_image(image, new_width, new_height)?;
                self.crop_center(scaled, target_width, target_height)
            }
            ScalingMode::Fit => {
                // Scale to fit entirely, add letterbox if needed
                let scale_x = target_width as f64 / img_width as f64;
                let scale_y = target_height as f64 / img_height as f64;
                let scale = scale_x.min(scale_y);

                let new_width = (img_width as f64 * scale) as u32;
                let new_height = (img_height as f64 * scale) as u32;

                let scaled = self.resize_image(image, new_width, new_height)?;
                self.center_on_background(scaled, target_width, target_height, [0, 0, 0, 255])
            }
            ScalingMode::Center => {
                // Center without scaling
                self.center_on_background(image, target_width, target_height, [0, 0, 0, 255])
            }
            ScalingMode::Tile => {
                // Tile the image
                self.tile_image(image, target_width, target_height)
            }
        }
    }

    fn resize_image(&self, image: RgbaImage, width: u32, height: u32) -> Result<RgbaImage> {
        use fast_image_resize as fr;

        let (src_width, src_height) = image.dimensions();

        if src_width == width && src_height == height {
            return Ok(image);
        }

        let src_image = fr::images::Image::from_vec_u8(
            src_width,
            src_height,
            image.into_raw(),
            fr::PixelType::U8x4,
        )?;

        let mut dst_image = fr::images::Image::new(width, height, fr::PixelType::U8x4);

        let mut resizer = fr::Resizer::new();
        resizer.resize(&src_image, &mut dst_image, &fr::ResizeOptions::default())?;

        let raw_pixels = dst_image.into_vec();
        Ok(RgbaImage::from_raw(width, height, raw_pixels)
            .ok_or_else(|| anyhow!("Failed to create image from resized data"))?)
    }

    fn crop_center(
        &self,
        image: RgbaImage,
        target_width: u32,
        target_height: u32,
    ) -> Result<RgbaImage> {
        let (img_width, img_height) = image.dimensions();

        if img_width == target_width && img_height == target_height {
            return Ok(image);
        }

        let x_offset = (img_width.saturating_sub(target_width)) / 2;
        let y_offset = (img_height.saturating_sub(target_height)) / 2;

        let mut result = RgbaImage::new(target_width, target_height);

        for y in 0..target_height {
            for x in 0..target_width {
                let src_x = x + x_offset;
                let src_y = y + y_offset;

                if src_x < img_width && src_y < img_height {
                    let pixel = image.get_pixel(src_x, src_y);
                    result.put_pixel(x, y, *pixel);
                }
            }
        }

        Ok(result)
    }

    fn center_on_background(
        &self,
        image: RgbaImage,
        target_width: u32,
        target_height: u32,
        bg_color: [u8; 4],
    ) -> Result<RgbaImage> {
        let (img_width, img_height) = image.dimensions();
        let mut result = RgbaImage::new(target_width, target_height);

        // Fill with background color
        for pixel in result.pixels_mut() {
            *pixel = image::Rgba(bg_color);
        }

        // Center the image
        let x_offset = (target_width.saturating_sub(img_width)) / 2;
        let y_offset = (target_height.saturating_sub(img_height)) / 2;

        for y in 0..img_height {
            for x in 0..img_width {
                let dst_x = x + x_offset;
                let dst_y = y + y_offset;

                if dst_x < target_width && dst_y < target_height {
                    let pixel = image.get_pixel(x, y);
                    result.put_pixel(dst_x, dst_y, *pixel);
                }
            }
        }

        Ok(result)
    }

    fn tile_image(
        &self,
        image: RgbaImage,
        target_width: u32,
        target_height: u32,
    ) -> Result<RgbaImage> {
        let (img_width, img_height) = image.dimensions();
        let mut result = RgbaImage::new(target_width, target_height);

        for y in 0..target_width {
            for x in 0..target_width {
                let src_x = x % img_width;
                let src_y = y % img_height;
                let pixel = image.get_pixel(src_x, src_y);
                result.put_pixel(x, y, *pixel);
            }
        }

        Ok(result)
    }

    fn compress_frame_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        let compressed = lz4_flex::compress_prepend_size(data);
        Ok(compressed)
    }

    pub fn decompress_frame_data(&self, compressed_data: &[u8]) -> Result<Vec<u8>> {
        let decompressed = lz4_flex::decompress_size_prepended(compressed_data)?;
        Ok(decompressed)
    }

    pub fn get_frame_data(&self, frame: &GifFrame) -> Result<Vec<u8>> {
        if frame.compressed {
            self.decompress_frame_data(&frame.data)
        } else {
            Ok(frame.data.clone())
        }
    }

    fn frame_delay_to_duration(delay: Delay) -> Duration {
        let (num, denom) = delay.numer_denom_ms();
        let ms = (num as f64 / denom as f64 * 1000.0) as u64;
        Duration::from_millis(ms.max(10)) // Minimum 10ms delay
    }

    fn get_cache_size(&self) -> usize {
        self.cache
            .values()
            .map(|frames| self.estimate_frames_size(frames))
            .sum()
    }

    fn estimate_frames_size(&self, frames: &[GifFrame]) -> usize {
        frames.iter().map(|frame| frame.data.len()).sum()
    }

    fn evict_oldest(&mut self) {
        if let Some(key) = self.cache.keys().next().cloned() {
            self.cache.remove(&key);
            debug!("Evicted cached GIF frames for: {}", key);
        }
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
        info!("Cleared GIF frame cache");
    }

    pub fn set_max_cache_size(&mut self, size: usize) {
        self.max_cache_size = size;

        // Evict if current cache is too large
        while self.get_cache_size() > self.max_cache_size && !self.cache.is_empty() {
            self.evict_oldest();
        }
    }

    pub fn cache_stats(&self) -> (usize, usize, usize) {
        let entries = self.cache.len();
        let size = self.get_cache_size();
        let total_frames: usize = self.cache.values().map(|frames| frames.len()).sum();

        (entries, size, total_frames)
    }
}

impl Default for GifCache {
    fn default() -> Self {
        Self::new()
    }
}

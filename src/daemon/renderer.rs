use anyhow::{anyhow, Result};
use fast_image_resize::IntoImageView;
use fast_image_resize::IntoImageViewMut;
use image::RgbaImage;
use log::{debug, info};
use memmap2::MmapMut;
use std::collections::HashMap;
use std::os::fd::{AsFd, BorrowedFd};
use std::sync::Arc;
use tempfile::NamedTempFile;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use wayland_client::{
    protocol::{wl_buffer, wl_shm, wl_surface},
    QueueHandle,
};

use crate::{
    cache::GifCache,
    config::{Config, ScalingMode},
    video::VideoPlayer,
    wayland::WaylandClient,
};

pub struct Renderer {
    config: Config,
    gif_cache: Arc<Mutex<GifCache>>,
    video_players: Arc<Mutex<HashMap<String, VideoPlayer>>>,
    animation_handles: HashMap<String, JoinHandle<()>>,
}

impl Renderer {
    pub fn new(config: Config) -> Result<Self> {
        Ok(Self {
            config,
            gif_cache: Arc::new(Mutex::new(GifCache::new())),
            video_players: Arc::new(Mutex::new(HashMap::new())),
            animation_handles: HashMap::new(),
        })
    }

    pub async fn render_image(
        &mut self,
        path: &str,
        width: u32,
        height: u32,
        mode: ScalingMode,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<WaylandClient>,
    ) -> Result<wl_buffer::WlBuffer> {
        let image_data = tokio::fs::read(path).await?;
        let image = image::load_from_memory(&image_data)?;
        let rgba_image = image.to_rgba8();

        let processed_image = self.process_image(rgba_image, width, height, mode)?;
        self.create_buffer_from_image(processed_image, shm, qh)
            .await
    }

    pub async fn start_gif(
        &mut self,
        path: &str,
        output: &str,
        width: u32,
        height: u32,
        mode: ScalingMode,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<WaylandClient>,
        surface: wl_surface::WlSurface,
    ) -> Result<()> {
        // Stop any existing animation for this output
        self.stop_content(output);

        // Load and cache GIF frames
        let frames = {
            let mut cache = self.gif_cache.lock().await;
            cache.get_or_load_frames(path, width, height, mode).await?
        };

        info!(
            "Starting GIF animation with {} frames on output {}",
            frames.len(),
            output
        );

        // For now, just set the first frame
        if let Some(first_frame) = frames.first() {
            let frame_data = {
                let cache = self.gif_cache.lock().await;
                cache.get_frame_data(first_frame)?
            };

            let buffer =
                Self::create_buffer_from_frame_data(&frame_data, width, height, shm, qh).await?;

            surface.attach(Some(&buffer), 0, 0);
            surface.damage_buffer(0, 0, width as i32, height as i32);
            surface.commit();
        }

        Ok(())
    }

    pub async fn start_video(
        &mut self,
        path: &str,
        output: &str,
        width: u32,
        height: u32,
        loop_video: bool,
        audio: bool,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<WaylandClient>,
        surface: wl_surface::WlSurface,
    ) -> Result<()> {
        // Stop any existing content for this output
        self.stop_content(output);

        // Create video player
        let video_player = VideoPlayer::new(path, width, height, loop_video, audio).await?;

        info!("Starting video playback on output {}", output);

        // Store the video player
        {
            let mut players = self.video_players.lock().await;
            players.insert(output.to_string(), video_player);
        }

        // For now, just create a black buffer as placeholder
        let buffer = self
            .create_solid_color_buffer(width, height, [0, 0, 0, 255], shm, qh)
            .await?;

        surface.attach(Some(&buffer), 0, 0);
        surface.damage_buffer(0, 0, width as i32, height as i32);
        surface.commit();

        Ok(())
    }

    pub fn stop_content(&mut self, output: &str) {
        if let Some(handle) = self.animation_handles.remove(output) {
            handle.abort();
            debug!("Stopped animation for output: {}", output);
        }

        // Remove video player if exists
        let video_players = self.video_players.clone();
        let output_name = output.to_string();
        tokio::spawn(async move {
            let mut players = video_players.lock().await;
            if let Some(mut player) = players.remove(&output_name) {
                player.stop().await;
                debug!("Stopped video player for output: {}", output_name);
            }
        });
    }

    pub async fn create_solid_color_buffer(
        &mut self,
        width: u32,
        height: u32,
        color: [u8; 4],
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<WaylandClient>,
    ) -> Result<wl_buffer::WlBuffer> {
        let mut image = RgbaImage::new(width, height);
        for pixel in image.pixels_mut() {
            *pixel = image::Rgba(color);
        }

        self.create_buffer_from_image(image, shm, qh).await
    }

    fn process_image(
        &self,
        image: RgbaImage,
        target_width: u32,
        target_height: u32,
        mode: ScalingMode,
    ) -> Result<RgbaImage> {
        let (img_width, img_height) = image.dimensions();

        match mode {
            ScalingMode::Fill => {
                // Scale to fill, crop excess
                let scale_x = target_width as f64 / img_width as f64;
                let scale_y = target_height as f64 / img_height as f64;
                let scale = scale_x.max(scale_y);

                let new_width = (img_width as f64 * scale) as u32;
                let new_height = (img_height as f64 * scale) as u32;

                let scaled = self.resize_image(image, new_width, new_height)?;

                // Crop to target size
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

                // Center on black background
                self.center_on_background(scaled, target_width, target_height, [0, 0, 0, 255])
            }
            ScalingMode::Center => {
                // Center without scaling
                if img_width == target_width && img_height == target_height {
                    Ok(image)
                } else {
                    self.center_on_background(image, target_width, target_height, [0, 0, 0, 255])
                }
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

        for y in 0..target_height {
            for x in 0..target_width {
                let src_x = x % img_width;
                let src_y = y % img_height;
                let pixel = image.get_pixel(src_x, src_y);
                result.put_pixel(x, y, *pixel);
            }
        }

        Ok(result)
    }

    async fn create_buffer_from_image(
        &mut self,
        image: RgbaImage,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<WaylandClient>,
    ) -> Result<wl_buffer::WlBuffer> {
        let (width, height) = image.dimensions();
        let stride = width * 4; // 4 bytes per pixel (RGBA)
        let size = stride * height;

        // Create a temporary file for shared memory
        let temp_file = NamedTempFile::new()?;
        temp_file.as_file().set_len(size as u64)?;

        // Create memory map
        let mut mmap = unsafe { MmapMut::map_mut(temp_file.as_file())? };

        // Convert RGBA to ARGB (Wayland expects ARGB)
        let image_data = image.into_raw();
        for i in (0..image_data.len()).step_by(4) {
            let r = image_data[i];
            let g = image_data[i + 1];
            let b = image_data[i + 2];
            let a = image_data[i + 3];

            // ARGB format
            mmap[i] = b; // Blue
            mmap[i + 1] = g; // Green
            mmap[i + 2] = r; // Red
            mmap[i + 3] = a; // Alpha
        }

        // Flush memory map
        mmap.flush()?;

        // Create Wayland shared memory pool
        let fd: BorrowedFd = temp_file.as_file().as_fd();
        let pool = shm.create_pool(fd, size as i32, qh, ());

        // Create buffer from pool
        let buffer = pool.create_buffer(
            0, // offset
            width as i32,
            height as i32,
            stride as i32,
            wl_shm::Format::Argb8888,
            qh,
            (),
        );

        Ok(buffer)
    }

    async fn create_buffer_from_frame_data(
        data: &[u8],
        width: u32,
        height: u32,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<WaylandClient>,
    ) -> Result<wl_buffer::WlBuffer> {
        let stride = width * 4;
        let size = stride * height;

        // Create a temporary file for shared memory
        let temp_file = NamedTempFile::new()?;
        temp_file.as_file().set_len(size as u64)?;

        // Create memory map
        let mut mmap = unsafe { MmapMut::map_mut(temp_file.as_file())? };

        // Copy frame data (assuming it's already in RGBA format)
        // Convert RGBA to ARGB for Wayland
        for i in (0..data.len().min(mmap.len())).step_by(4) {
            let r = data[i];
            let g = data[i + 1];
            let b = data[i + 2];
            let a = data[i + 3];

            // ARGB format
            mmap[i] = b; // Blue
            mmap[i + 1] = g; // Green
            mmap[i + 2] = r; // Red
            mmap[i + 3] = a; // Alpha
        }

        // Flush memory map
        mmap.flush()?;

        // Create Wayland shared memory pool
        let fd: BorrowedFd = temp_file.as_file().as_fd();
        let pool = shm.create_pool(fd, size as i32, qh, ());

        // Create buffer from pool
        let buffer = pool.create_buffer(
            0, // offset
            width as i32,
            height as i32,
            stride as i32,
            wl_shm::Format::Argb8888,
            qh,
            (),
        );

        Ok(buffer)
    }
}

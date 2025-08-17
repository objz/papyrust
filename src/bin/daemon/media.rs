use anyhow::{Result, anyhow};
use ffmpeg_next as ffmpeg;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use crate::gl_bindings as gl;
use crate::lossless_scaling::{LosslessScaler, ScalingAlgorithm};

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

    #[allow(dead_code)]
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

pub struct VideoDecoder {
    decoder: ffmpeg::decoder::Video,
    scaler: Option<ffmpeg::software::scaling::Context>,
    texture: u32,
    _width: u32,
    _height: u32,
    input_ctx: ffmpeg::format::context::Input,
    stream_index: usize,
    video_path: String,
    last_frame_updated: bool,
    _time_base: f64,
    _video_start_time: f64,
    _playback_start_time: f64,
    forced_fps: Option<f64>,
    frame_count: u64,
    last_forced_frame_time: f64,
    current_frame: Option<ffmpeg::frame::Video>,
    next_frame: Option<ffmpeg::frame::Video>,
    reached_eof: bool,
    _video_fps: f64,
    _video_duration: f64,
    loop_count: u64,
    _lossless_scaler: Option<Arc<LosslessScaler>>,
}

impl VideoDecoder {
    pub fn new(path: &str) -> Result<Self> {
        Self::new_with_fps(path, None)
    }

    pub fn new_with_fps(path: &str, forced_fps: Option<f64>) -> Result<Self> {
        Self::new_with_scaler(path, forced_fps, None)
    }

    #[allow(dead_code)]
    pub async fn new_with_lossless_scaling(
        path: &str,
        forced_fps: Option<f64>,
        algorithm: ScalingAlgorithm,
    ) -> Result<Self> {
        let scaler = LosslessScaler::new(algorithm).await?;
        Self::new_with_scaler(path, forced_fps, Some(Arc::new(scaler)))
    }

    fn new_with_scaler(
        path: &str,
        forced_fps: Option<f64>,
        lossless_scaler: Option<Arc<LosslessScaler>>,
    ) -> Result<Self> {
        let fps_msg = if let Some(fps) = forced_fps {
            format!("forced FPS: {:.1}", fps)
        } else {
            "original timing".to_string()
        };
        tracing::info!(event = "video_open", path = %path, %fps_msg, has_lossless = lossless_scaler.is_some(), "Initializing video decoder");

        ffmpeg::init().map_err(|e| anyhow!("Failed to initialize FFmpeg: {}", e))?;
        let input_ctx = ffmpeg::format::input(&Path::new(path))
            .map_err(|e| anyhow!("Failed to open video file {}: {}", path, e))?;

        let stream = input_ctx
            .streams()
            .best(ffmpeg::media::Type::Video)
            .ok_or_else(|| anyhow!("No video stream found in {}", path))?;
        let stream_index = stream.index();

        let context_decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
            .map_err(|e| anyhow!("Failed to create codec context: {}", e))?;
        let decoder = context_decoder
            .decoder()
            .video()
            .map_err(|e| anyhow!("Failed to create video decoder: {}", e))?;

        let width = decoder.width();
        let height = decoder.height();

        let time_base = {
            let tb = stream.time_base();
            tb.0 as f64 / tb.1 as f64
        };

        let video_start_time = {
            let start = stream.start_time();
            if start != ffmpeg::ffi::AV_NOPTS_VALUE {
                start as f64 * time_base
            } else {
                0.0
            }
        };

        let video_duration = {
            let duration = stream.duration();
            if duration != ffmpeg::ffi::AV_NOPTS_VALUE {
                duration as f64 * time_base
            } else {
                let format_duration = input_ctx.duration();
                if format_duration != ffmpeg::ffi::AV_NOPTS_VALUE {
                    format_duration as f64 / ffmpeg::ffi::AV_TIME_BASE as f64
                } else {
                    0.0
                }
            }
        };

        let video_fps = {
            let rate = stream.rate();
            let fps = if rate.1 > 0 {
                rate.0 as f64 / rate.1 as f64
            } else {
                let avg_rate = stream.avg_frame_rate();
                if avg_rate.1 > 0 {
                    avg_rate.0 as f64 / avg_rate.1 as f64
                } else {
                    1.0 / time_base
                }
            };
            if (0.1..=240.0).contains(&fps) {
                fps
            } else {
                tracing::warn!(
                    event = "video_unusual_fps",
                    fps,
                    "Unusual FPS; using time base"
                );
                1.0 / time_base
            }
        };

        tracing::info!(
            event = "video_info",
            width,
            height,
            fps = video_fps,
            forced_fps,
            duration = video_duration,
            frame_duration = 1.0 / video_fps,
            "Video stream initialized"
        );

        let scaler = if decoder.format() != ffmpeg::format::Pixel::RGBA {
            Some(
                ffmpeg::software::scaling::Context::get(
                    decoder.format(),
                    width,
                    height,
                    ffmpeg::format::Pixel::RGBA,
                    width,
                    height,
                    ffmpeg::software::scaling::flag::Flags::LANCZOS,
                )
                .map_err(|e| anyhow!("Failed to create scaler: {}", e))?,
            )
        } else {
            None
        };

        let mut texture = 0;
        unsafe {
            gl::GenTextures(1, &mut texture);
            gl::BindTexture(gl::TEXTURE_2D, texture);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);

            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);

            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA as i32,
                width as i32,
                height as i32,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                std::ptr::null(),
            );
        }

        Ok(Self {
            decoder,
            scaler,
            texture,
            _width: width,
            _height: height,
            input_ctx,
            stream_index,
            video_path: path.to_string(),
            last_frame_updated: false,
            _time_base: time_base,
            _video_start_time: video_start_time,
            _playback_start_time: crate::utils::get_time_millis() as f64 / 1000.0,
            forced_fps,
            frame_count: 0,
            last_forced_frame_time: crate::utils::get_time_millis() as f64 / 1000.0,
            current_frame: None,
            next_frame: None,
            reached_eof: false,
            _video_fps: video_fps,
            _video_duration: video_duration,
            loop_count: 0,
            _lossless_scaler: lossless_scaler,
        })
    }

    pub fn update_frame(&mut self) -> Result<bool> {
        self.last_frame_updated = false;
        let current_time = crate::utils::get_time_millis() as f64 / 1000.0;

        if let Some(forced_fps) = self.forced_fps {
            let min_frame_duration = 1.0 / forced_fps;
            let elapsed = current_time - self.last_forced_frame_time;

            if elapsed < min_frame_duration {
                return Ok(false);
            }

            self.last_forced_frame_time = current_time;
        }

        if self.current_frame.is_some() {
            if self.next_frame.is_some() {
                self.current_frame = self.next_frame.take();
                self.upload_current_frame();
                self.last_frame_updated = true;
                self.frame_count += 1;

                self.decode_next_frame()?;
                return Ok(self.last_frame_updated);
            } else {
                self.decode_next_frame()?;
                return Ok(self.last_frame_updated);
            }
        } else {
            self.decode_next_frame()?;
            if self.next_frame.is_some() {
                self.current_frame = self.next_frame.take();
                self.upload_current_frame();
                self.last_frame_updated = true;
                self.frame_count += 1;
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn decode_next_frame(&mut self) -> Result<()> {
        if !self.decode_frame_to_buffer()? {
            self.loop_count += 1;
            tracing::debug!(
                event = "video_loop",
                loop_count = self.loop_count,
                "Video restarted for loop"
            );
            self.restart_video()?;
            self.decode_frame_to_buffer()?;
        }
        Ok(())
    }

    fn decode_frame_to_buffer(&mut self) -> Result<bool> {
        if self.reached_eof {
            return Ok(false);
        }

        for (stream, packet) in self.input_ctx.packets() {
            if stream.index() != self.stream_index {
                continue;
            }

            match self.decoder.send_packet(&packet) {
                Ok(_) => {
                    let mut decoded = ffmpeg::frame::Video::empty();
                    while self.decoder.receive_frame(&mut decoded).is_ok() {
                        let rgba_frame = self.convert_frame(decoded)?;
                        self.next_frame = Some(rgba_frame);
                        return Ok(true);
                    }
                }
                Err(ffmpeg::Error::Eof) => {
                    self.reached_eof = true;
                    return Ok(false);
                }
                Err(_) => {
                    continue;
                }
            }
        }

        self.reached_eof = true;
        Ok(false)
    }

    fn convert_frame(&mut self, frame: ffmpeg::frame::Video) -> Result<ffmpeg::frame::Video> {
        if frame.format() != ffmpeg::format::Pixel::RGBA {
            if let Some(ref mut scaler) = self.scaler {
                let mut rgba_frame = ffmpeg::frame::Video::empty();
                scaler
                    .run(&frame, &mut rgba_frame)
                    .map_err(|e| anyhow!("Scaling failed: {}", e))?;
                rgba_frame.set_pts(frame.pts());
                Ok(rgba_frame)
            } else {
                Ok(frame)
            }
        } else {
            Ok(frame)
        }
    }

    fn upload_current_frame(&self) {
        if let Some(ref frame) = self.current_frame {
            self.upload_frame(frame);
        }
    }

    fn upload_frame(&self, frame: &ffmpeg::frame::Video) {
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, self.texture);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                0,
                0,
                frame.width() as i32,
                frame.height() as i32,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                frame.data(0).as_ptr() as *const _,
            );
        }
    }

    fn restart_video(&mut self) -> Result<()> {
        self.current_frame = None;
        self.next_frame = None;
        self.reached_eof = false;

        self.input_ctx = ffmpeg::format::input(&Path::new(&self.video_path))
            .map_err(|e| anyhow!("Failed to re-open video {}: {}", self.video_path, e))?;

        let stream = self
            .input_ctx
            .streams()
            .best(ffmpeg::media::Type::Video)
            .ok_or_else(|| anyhow!("No video stream on restart"))?;

        self.stream_index = stream.index();

        let context_decoder =
            ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
        self.decoder = context_decoder.decoder().video()?;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn scale_frame_with_lossless(
        &self,
        frame_data: &[u8],
        target_width: u32,
        target_height: u32,
        sharpening: f32,
    ) -> Result<Vec<u8>> {
        if let Some(ref scaler) = self._lossless_scaler {
            scaler.scale_texture(
                frame_data,
                self._width,
                self._height,
                target_width,
                target_height,
                sharpening,
            )
        } else {
            Ok(frame_data.to_vec())
        }
    }

    pub fn texture(&self) -> u32 {
        self.texture
    }

    pub fn width(&self) -> u32 {
        self._width
    }

    pub fn height(&self) -> u32 {
        self._height
    }

    pub fn has_new_frame(&self) -> bool {
        self.last_frame_updated
    }
}

static IMAGE_LOADER: OnceLock<ImageLoader> = OnceLock::new();

fn get_image_loader() -> &'static ImageLoader {
    IMAGE_LOADER.get_or_init(|| ImageLoader::new())
}

pub fn load_texture(path: &str) -> Result<u32> {
    get_image_loader().load_texture(path)
}

pub fn load_shader(path: &str) -> Result<String> {
    let mut file =
        File::open(path).map_err(|e| anyhow!("Failed to open shader file {}: {}", path, e))?;
    let mut source = String::new();
    file.read_to_string(&mut source)
        .map_err(|e| anyhow!("Failed to read shader file {}: {}", path, e))?;
    Ok(source)
}

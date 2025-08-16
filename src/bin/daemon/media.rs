use anyhow::{anyhow, Result};
use ffmpeg_next as ffmpeg;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::gl_bindings as gl;

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

pub struct ImageLoader;

impl ImageLoader {
    pub fn load_texture(path: &str) -> Result<u32> {
        eprintln!("Loading image: {}", path);

        let img = image::open(path).map_err(|e| anyhow!("Failed to load image {}: {}", path, e))?;
        let rgba = img.to_rgba8();
        let (width, height) = (img.width(), img.height());

        eprintln!("Image loaded: {}x{}", width, height);

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
                rgba.as_ptr() as *const _,
            );
        }

        Ok(texture)
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
    last_frame_time: f64,
    frame_duration_ms: f64,
    accumulated_time: f64,
    last_frame_updated: bool,
}

impl VideoDecoder {
    pub fn new(path: &str) -> Result<Self> {
        eprintln!("Initializing video decoder for: {}", path);

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

        let video_fps = {
            let rate = stream.rate();
            let fps = if rate.1 > 0 {
                rate.0 as f64 / rate.1 as f64
            } else {
                let tb = stream.time_base();
                if tb.1 > 0 {
                    tb.1 as f64 / tb.0 as f64
                } else {
                    30.0
                }
            };
            if (1.0..=240.0).contains(&fps) {
                fps
            } else {
                eprintln!("Warning: Unusual FPS ({:.2}), defaulting to 30", fps);
                30.0
            }
        };
        let frame_duration_ms = (1000.0 / video_fps).max(1.0);

        let scaler = if decoder.format() != ffmpeg::format::Pixel::RGB24 {
            Some(
                ffmpeg::software::scaling::Context::get(
                    decoder.format(),
                    width,
                    height,
                    ffmpeg::format::Pixel::RGB24,
                    width,
                    height,
                    ffmpeg::software::scaling::flag::Flags::BILINEAR,
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
                gl::RGB as i32,
                width as i32,
                height as i32,
                0,
                gl::RGB,
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
            last_frame_time: crate::utils::get_time_millis() as f64,
            frame_duration_ms,
            accumulated_time: 0.0,
            last_frame_updated: false,
        })
    }

    pub fn update_frame(&mut self) -> Result<bool> {
        // Reset frame update flag
        self.last_frame_updated = false;

        // Timing
        let now = crate::utils::get_time_millis() as f64;
        let dt = now - self.last_frame_time;
        self.accumulated_time += dt;
        self.last_frame_time = now;
        if self.accumulated_time < self.frame_duration_ms {
            return Ok(false);
        }
        self.accumulated_time -= self.frame_duration_ms;

        // Try to read packets & decode
        let mut saw_any = false;
        for (stream, packet) in self.input_ctx.packets() {
            if stream.index() != self.stream_index {
                continue;
            }
            saw_any = true;
            match self.decoder.send_packet(&packet) {
                Ok(_) => {
                    let mut decoded = ffmpeg::frame::Video::empty();
                    while self.decoder.receive_frame(&mut decoded).is_ok() {
                        let rgb_frame = if decoded.format() != ffmpeg::format::Pixel::RGB24 {
                            if let Some(ref mut scaler) = self.scaler {
                                let mut out = ffmpeg::frame::Video::empty();
                                scaler.run(&decoded, &mut out)
                                      .map_err(|e| anyhow!("Scaling failed: {}", e))?;
                                out
                            } else {
                                decoded.clone()
                            }
                        } else {
                            decoded.clone()
                        };
                        unsafe {
                            gl::BindTexture(gl::TEXTURE_2D, self.texture);
                            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
                            gl::TexSubImage2D(
                                gl::TEXTURE_2D,
                                0,
                                0,
                                0,
                                rgb_frame.width() as i32,
                                rgb_frame.height() as i32,
                                gl::RGB,
                                gl::UNSIGNED_BYTE,
                                rgb_frame.data(0).as_ptr() as *const _,
                            );
                        }
                        self.last_frame_updated = true;
                        return Ok(true);
                    }
                }
                Err(ffmpeg::Error::Eof) => {
                    self.restart_video()?;
                    return self.update_frame();
                }
                Err(_) => continue,
            }
        }

        // If no packets left, rewind
        if !saw_any {
            self.restart_video()?;
            return self.update_frame();
        }

        Ok(false)
    }

    fn restart_video(&mut self) -> Result<()> {
        if let Err(_) = self.input_ctx.seek(0, 0..i64::MAX) {
            eprintln!("Seek failed; re-opening video");
            self.input_ctx = ffmpeg::format::input(&Path::new(&self.video_path))
                .map_err(|e| anyhow!("Failed to re-open video {}: {}", self.video_path, e))?;
            let stream = self.input_ctx
                .streams()
                .best(ffmpeg::media::Type::Video)
                .ok_or_else(|| anyhow!("No video stream on restart"))?;
            let context_decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
            self.decoder = context_decoder.decoder().video()?;
        }
        self.accumulated_time = 0.0;
        self.last_frame_time = crate::utils::get_time_millis() as f64;
        Ok(())
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

pub fn load_shader(path: &str) -> Result<String> {
    let mut file = File::open(path).map_err(|e| anyhow!("Failed to open shader file {}: {}", path, e))?;
    let mut source = String::new();
    file.read_to_string(&mut source)
        .map_err(|e| anyhow!("Failed to read shader file {}: {}", path, e))?;
    Ok(source)
}

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

            eprintln!("Texture created successfully: {}", texture);
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
    eof_reached: bool,
    video_fps: f64,
    last_frame_time: u64,
    frame_duration_ms: u64,
    accumulated_time: f64, 
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
                let time_base = stream.time_base();
                if time_base.1 > 0 {
                    time_base.1 as f64 / time_base.0 as f64
                } else {
                    30.0 
                }
            };
            
            if fps >= 1.0 && fps <= 240.0 {
                fps
            } else {
                eprintln!("Warning: Detected unusual FPS ({:.2}), using default 30 FPS", fps);
                30.0
            }
        };

        let frame_duration_ms = (1000.0 / video_fps) as u64;
        
        let frame_duration_ms = frame_duration_ms.max(1);

        eprintln!("Video info: {}x{}, FPS: {:.2}, frame duration: {}ms", width, height, video_fps, frame_duration_ms);

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
            eof_reached: false,
            video_fps,
            last_frame_time: crate::utils::get_time_millis(),
            frame_duration_ms,
            accumulated_time: 0.0,
        })
    }

    pub fn update_frame(&mut self) -> Result<bool> {
        let current_time = crate::utils::get_time_millis();
        let delta_time = current_time - self.last_frame_time;
        
        self.accumulated_time += delta_time as f64;
        
        if self.accumulated_time < self.frame_duration_ms as f64 {
            return Ok(false); 
        }

        let mut frame_updated = false;

        // restart the video
        if self.eof_reached {
            self.restart_video()?;
            self.eof_reached = false;
        }

        for (stream, packet) in self.input_ctx.packets() {
            if stream.index() == self.stream_index {
                match self.decoder.send_packet(&packet) {
                    Ok(_) => {
                        let mut decoded_frame = ffmpeg::frame::Video::empty();

                        while self.decoder.receive_frame(&mut decoded_frame).is_ok() {
                            let rgb_frame = if decoded_frame.format()
                                != ffmpeg::format::Pixel::RGB24
                            {
                                if let Some(ref mut scaler) = self.scaler {
                                    let mut rgb_frame = ffmpeg::frame::Video::empty();
                                    scaler
                                        .run(&decoded_frame, &mut rgb_frame)
                                        .map_err(|e| anyhow!("Scaling failed: {}", e))?;
                                    rgb_frame
                                } else {
                                    let mut new_scaler = ffmpeg::software::scaling::Context::get(
                                        decoded_frame.format(),
                                        decoded_frame.width(),
                                        decoded_frame.height(),
                                        ffmpeg::format::Pixel::RGB24,
                                        decoded_frame.width(),
                                        decoded_frame.height(),
                                        ffmpeg::software::scaling::flag::Flags::BILINEAR,
                                    )
                                    .map_err(|e| anyhow!("Failed to create scaler: {}", e))?;

                                    let mut rgb_frame = ffmpeg::frame::Video::empty();
                                    new_scaler
                                        .run(&decoded_frame, &mut rgb_frame)
                                        .map_err(|e| anyhow!("Scaling failed: {}", e))?;
                                    self.scaler = Some(new_scaler);
                                    rgb_frame
                                }
                            } else {
                                decoded_frame
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

                            frame_updated = true;
                            self.last_frame_time = current_time;
                            self.accumulated_time -= self.frame_duration_ms as f64;
                            return Ok(frame_updated);
                        }
                    }
                    Err(ffmpeg::Error::Eof) => {
                        self.eof_reached = true;
                        break;
                    }
                    Err(_) => {
                        continue;
                    }
                }
            }
        }

        if !frame_updated {
            self.eof_reached = true;
        }

        Ok(frame_updated)
    }

    fn restart_video(&mut self) -> Result<()> {
        self.last_frame_time = crate::utils::get_time_millis();
        self.accumulated_time = 0.0;
        
        if let Err(_) = self.input_ctx.seek(0, 0..i64::MAX) {
            eprintln!("Seeking failed, recreating input context");

            self.input_ctx = ffmpeg::format::input(&Path::new(&self.video_path))
                .map_err(|e| anyhow!("Failed to reopen video file: {}", e))?;

            let stream = self
                .input_ctx
                .streams()
                .best(ffmpeg::media::Type::Video)
                .ok_or_else(|| anyhow!("No video stream found after restart"))?;

            self.stream_index = stream.index();

            let context_decoder =
                ffmpeg::codec::context::Context::from_parameters(stream.parameters())
                    .map_err(|e| anyhow!("Failed to recreate codec context: {}", e))?;

            self.decoder = context_decoder
                .decoder()
                .video()
                .map_err(|e| anyhow!("Failed to recreate video decoder: {}", e))?;
        }

        Ok(())
    }

    pub fn texture(&self) -> u32 {
        self.texture
    }

    pub fn _dimensions(&self) -> (u32, u32) {
        (self._width, self._height)
    }

    pub fn fps(&self) -> f64 {
        self.video_fps
    }
}

pub fn load_shader(path: &str) -> Result<String> {
    let mut file =
        File::open(path).map_err(|e| anyhow!("Failed to open shader file {}: {}", path, e))?;
    let mut source = String::new();
    file.read_to_string(&mut source)
        .map_err(|e| anyhow!("Failed to read shader file {}: {}", path, e))?;
    Ok(source)
}

pub fn default_shader() -> &'static str {
    r#"
#ifdef GL_ES
precision mediump float;
#endif

uniform sampler2D u_media;
uniform vec2 u_resolution;
uniform float u_time;

varying vec2 texCoords;

void main() {
    // Simple passthrough with optional UV animation
    vec2 uv = texCoords;
    
    // Subtle breathing effect
    float scale = 1.0 + 0.01 * sin(u_time * 2.0);
    uv = (uv - 0.5) * scale + 0.5;
    
    vec4 color = texture2D(u_media, uv);
    gl_FragColor = color;
}
"#
}

pub fn vertex_shader() -> &'static str {
    r#"
#version 100
attribute vec2 datIn;
attribute vec2 texIn;
varying vec2 texCoords;

void main() {
    texCoords = texIn;
    gl_Position = vec4(datIn, 0.0, 1.0);
}
"#
}

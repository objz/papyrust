use anyhow::{Result, anyhow};
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
        tracing::info!(event = "image_load", path = %path, "Loading image");

        let img = image::open(path).map_err(|e| anyhow!("Failed to load image {}: {}", path, e))?;
        let rgba = img.to_rgba8();
        let (width, height) = (img.width(), img.height());

        tracing::debug!(event = "image_info", width, height, "Image decoded");

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
    last_frame_updated: bool,
    time_base: f64,
    video_start_time: f64,
    playback_start_time: f64,
    forced_fps: Option<f64>,
    frame_count: u64,
    last_forced_frame_time: f64,
    current_frame: Option<ffmpeg::frame::Video>,
    next_frame: Option<ffmpeg::frame::Video>,
    reached_eof: bool,
    video_fps: f64,
    video_duration: f64,
    loop_count: u64,
}

impl VideoDecoder {
    pub fn new(path: &str) -> Result<Self> {
        Self::new_with_fps(path, None)
    }

    pub fn new_with_fps(path: &str, forced_fps: Option<f64>) -> Result<Self> {
        let fps_msg = if let Some(fps) = forced_fps {
            format!("forced FPS: {:.1}", fps)
        } else {
            "original timing".to_string()
        };
        tracing::info!(event = "video_open", path = %path, %fps_msg, "Initializing video decoder");

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
            last_frame_updated: false,
            time_base,
            video_start_time,
            playback_start_time: crate::utils::get_time_millis() as f64 / 1000.0,
            forced_fps,
            frame_count: 0,
            last_forced_frame_time: crate::utils::get_time_millis() as f64 / 1000.0,
            current_frame: None,
            next_frame: None,
            reached_eof: false,
            video_fps,
            video_duration,
            loop_count: 0,
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

        if let Some(ref frame) = self.current_frame {
            if let Some(ref next_frame) = self.next_frame {
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
                        let rgb_frame = self.convert_frame(decoded)?;
                        self.next_frame = Some(rgb_frame);
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
        if frame.format() != ffmpeg::format::Pixel::RGB24 {
            if let Some(ref mut scaler) = self.scaler {
                let mut rgb_frame = ffmpeg::frame::Video::empty();
                scaler
                    .run(&frame, &mut rgb_frame)
                    .map_err(|e| anyhow!("Scaling failed: {}", e))?;
                rgb_frame.set_pts(frame.pts());
                Ok(rgb_frame)
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
                gl::RGB,
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
    let mut file =
        File::open(path).map_err(|e| anyhow!("Failed to open shader file {}: {}", path, e))?;
    let mut source = String::new();
    file.read_to_string(&mut source)
        .map_err(|e| anyhow!("Failed to read shader file {}: {}", path, e))?;
    Ok(source)
}

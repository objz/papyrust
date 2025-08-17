use anyhow::{Result, anyhow};
use ffmpeg_next as ffmpeg;
use std::path::Path;

use crate::gl_bindings as gl;

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
    playback_start_time: f64,
    forced_fps: Option<f64>,
    current_frame: Option<ffmpeg::frame::Video>,
    next_frame: Option<ffmpeg::frame::Video>,
    current_frame_pts: Option<i64>,
    next_frame_pts: Option<i64>,
    reached_eof: bool,
    video_fps: f64,
    video_duration: f64,
    loop_count: u64,
    first_pts: Option<i64>,
    frame_count: u64,
}

impl VideoDecoder {
    pub fn new(path: &str) -> Result<Self> {
        Self::new_with_fps(path, None)
    }

    pub fn new_with_fps(path: &str, forced_fps: Option<f64>) -> Result<Self> {
        Self::new_with_scaler(path, forced_fps)
    }

    fn new_with_scaler(path: &str, forced_fps: Option<f64>) -> Result<Self> {
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
            let avg_rate = stream.avg_frame_rate();

            let fps_from_rate = if rate.1 > 0 {
                rate.0 as f64 / rate.1 as f64
            } else {
                0.0
            };

            let fps_from_avg = if avg_rate.1 > 0 {
                avg_rate.0 as f64 / avg_rate.1 as f64
            } else {
                0.0
            };

            let detected_fps = if (1.0..=120.0).contains(&fps_from_rate) {
                fps_from_rate
            } else if (1.0..=120.0).contains(&fps_from_avg) {
                fps_from_avg
            } else if time_base > 0.0 {
                let tb_fps = 1.0 / time_base;
                if (1.0..=120.0).contains(&tb_fps) {
                    tb_fps
                } else {
                    25.0
                }
            } else {
                25.0
            };

            tracing::debug!(
                event = "fps_detection",
                rate_fps = fps_from_rate,
                avg_fps = fps_from_avg,
                time_base_fps = if time_base > 0.0 {
                    1.0 / time_base
                } else {
                    0.0
                },
                detected_fps,
                "FPS detection results"
            );

            detected_fps
        };

        tracing::info!(
            event = "video_info",
            width,
            height,
            fps = video_fps,
            forced_fps,
            duration = video_duration,
            time_base,
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
            time_base,
            playback_start_time: crate::utils::get_time_millis() as f64 / 1000.0,
            forced_fps,
            current_frame: None,
            next_frame: None,
            current_frame_pts: None,
            next_frame_pts: None,
            reached_eof: false,
            video_fps,
            video_duration,
            loop_count: 0,
            first_pts: None,
            frame_count: 0,
        })
    }

    pub fn update_frame(&mut self) -> Result<bool> {
        self.last_frame_updated = false;
        let current_time = crate::utils::get_time_millis() as f64 / 1000.0;
        let playback_time = current_time - self.playback_start_time;

        if self.next_frame.is_none() {
            self.decode_next_frame()?;
        }

        let should_display_next = if let Some(forced_fps) = self.forced_fps {
            let frame_duration = 1.0 / forced_fps;
            let expected_frame_time = self.frame_count as f64 * frame_duration;
            playback_time >= expected_frame_time
        } else {
            if let Some(next_pts) = self.next_frame_pts {
                let frame_time = self.pts_to_time(next_pts);
                playback_time >= frame_time
            } else {
                let frame_duration = 1.0 / self.video_fps;
                let expected_frame_time = self.frame_count as f64 * frame_duration;
                playback_time >= expected_frame_time
            }
        };

        if should_display_next && self.next_frame.is_some() {
            self.current_frame = self.next_frame.take();
            self.current_frame_pts = self.next_frame_pts.take();

            if let Some(ref frame) = self.current_frame {
                self.upload_frame(frame);
                self.last_frame_updated = true;
                self.frame_count += 1;

                self.decode_next_frame()?;
            }
        }

        Ok(self.last_frame_updated)
    }

    fn pts_to_time(&self, pts: i64) -> f64 {
        let adjusted_pts = if let Some(first) = self.first_pts {
            pts - first
        } else {
            pts
        };
        adjusted_pts as f64 * self.time_base
    }

    fn decode_next_frame(&mut self) -> Result<()> {
        if self.next_frame.is_some() {
            return Ok(());
        }

        if !self.decode_frame_to_buffer()? {
            self.loop_count += 1;
            tracing::debug!(
                event = "video_loop",
                loop_count = self.loop_count,
                frame_count = self.frame_count,
                "Video restarted for loop"
            );

            let expected_loop_duration = if let Some(forced_fps) = self.forced_fps {
                self.frame_count as f64 / forced_fps
            } else if self.video_duration > 0.0 {
                self.video_duration
            } else {
                self.frame_count as f64 / self.video_fps
            };

            let current_time = crate::utils::get_time_millis() as f64 / 1000.0;
            self.playback_start_time =
                current_time - (self.loop_count as f64 * expected_loop_duration);

            self.frame_count = 0;
            self.first_pts = None;

            self.restart_video()?;
            self.decode_frame_to_buffer()?;

            tracing::debug!(
                event = "video_loop_timing",
                expected_duration = expected_loop_duration,
                new_start_time = self.playback_start_time,
                "Adjusted timing for seamless loop"
            );
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
                        let pts = decoded.pts();

                        if let Some(pts_val) = pts {
                            if self.first_pts.is_none() {
                                self.first_pts = Some(pts_val);
                                tracing::debug!(
                                    event = "video_first_pts",
                                    pts = pts_val,
                                    time = self.pts_to_time(pts_val),
                                    "First frame PTS recorded"
                                );
                            }
                        }

                        let rgba_frame = self.convert_frame(decoded)?;
                        self.next_frame = Some(rgba_frame);
                        self.next_frame_pts = pts;
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
        self.current_frame_pts = None;
        self.next_frame_pts = None;
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

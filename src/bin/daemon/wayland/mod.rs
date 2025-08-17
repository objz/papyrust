use crate::ipc::MediaChange;
use crate::media::MediaType;
use crate::utils;
use anyhow::{Result, anyhow};
use std::process::Child;
use std::sync::mpsc::Receiver;
use wayland_client::Connection;

pub mod audio;
pub mod monitors;
pub mod protocol;
pub mod rendering;
pub mod traits;
pub mod types;

use audio::fifo::FifoReader;
use monitors::MonitorManager;
use traits::WaylandSurface as WaylandSurfaceTrait;
use types::WaylandConfig;

struct WaylandManager {
    monitor_manager: MonitorManager,
    config: WaylandConfig,
}

impl WaylandManager {
    fn new(config: WaylandConfig) -> Self {
        Self {
            monitor_manager: MonitorManager::new(),
            config,
        }
    }

    fn initialize(&mut self, conn: &Connection) -> Result<()> {
        let mut event_queue = conn.new_event_queue();
        let qh = event_queue.handle();
        let mut app_state = protocol::events::AppState::new();

        let _registry = conn.display().get_registry(&qh, ());
        event_queue.roundtrip(&mut app_state)?;

        if let Some(ref om) = app_state.output_manager {
            for (id, info) in &app_state.outputs {
                om.get_xdg_output(&info.output, &qh, *id);
            }
        }
        event_queue.roundtrip(&mut app_state)?;

        let compositor = app_state
            .compositor
            .as_ref()
            .ok_or_else(|| anyhow!("Compositor not available"))?;
        let layer_shell = app_state
            .layer_shell
            .as_ref()
            .ok_or_else(|| anyhow!("Layer shell not available"))?;

        let mut total_surfaces = 0;
        for output_info in app_state.outputs.values() {
            if output_info.name.is_some() {
                self.monitor_manager.create_surface(
                    output_info,
                    compositor,
                    layer_shell,
                    self.config.layer_name.as_deref(),
                    MediaType::Shader("default".to_string()),
                    conn,
                    &qh,
                    self.config.fps,
                )?;
                total_surfaces += 1;
            }
        }

        event_queue.roundtrip(&mut app_state)?;
        while app_state.configured_count < total_surfaces {
            tracing::debug!(
                event = "waiting_layer_config",
                configured = app_state.configured_count,
                total = total_surfaces,
                "Awaiting layer surface configuration"
            );
            event_queue.blocking_dispatch(&mut app_state)?;
        }
        event_queue.roundtrip(&mut app_state)?;

        for surface in self.monitor_manager.surfaces_mut() {
            if let Some((width, height)) =
                app_state.layer_surface_configs.get(&surface.surface_id.0)
            {
                tracing::info!(
                    event = "surface_configured",
                    output = %surface.output_name,
                    width, height,
                    "Applying initial layer surface config"
                );
                WaylandSurfaceTrait::resize(surface, *width, *height)?;
            }
        }

        Ok(())
    }
}

pub fn init(
    media_type: MediaType,
    fps: u16,
    layer_name: Option<&str>,
    fifo_path: Option<&str>,
    ipc_receiver: Receiver<MediaChange>,
    mute: bool,
) -> Result<()> {
    tracing::info!(
        event = "wayland_init",
        fps,
        layer = layer_name,
        fifo = fifo_path,
        mute,
        "Initializing Wayland stack"
    );

    let config = WaylandConfig {
        fps,
        layer_name: layer_name.map(String::from),
    };

    let conn = Connection::connect_to_env()?;
    let mut wayland_manager = WaylandManager::new(config);
    wayland_manager.initialize(&conn)?;

    let mut fifo_reader = fifo_path.map(FifoReader::new).transpose()?;
    let mut has_video = matches!(media_type, MediaType::Video { .. });
    let mut current_media_type = media_type.clone();

    wayland_manager
        .monitor_manager
        .set_swap_intervals(has_video, fps)?;

    tracing::info!(
        event = "render_loop_start",
        monitors = wayland_manager.monitor_manager.len(),
        "Starting render loop"
    );

    let mut last_audio_path: Option<String> = None;
    let mut last_audio_child: Option<Child> = None;
    let mut frame_count = 0u64;
    let mut last_fps_check = utils::get_time_millis();

    let base_frame_time = if fps > 0 { 1000 / fps as u64 } else { 16 };

    loop {
        let frame_start = utils::get_time_millis();

        if let Ok(media_change) = ipc_receiver.try_recv() {
            let new_has_video = matches!(media_change.media_type, MediaType::Video { .. });
            if has_video != new_has_video {
                has_video = new_has_video;
                wayland_manager
                    .monitor_manager
                    .set_swap_intervals(has_video, fps)?;
                tracing::info!(
                    event = "swap_interval_reconfigured",
                    has_video,
                    "Reconfigured swap intervals due to media type change"
                );
            }

            current_media_type = media_change.media_type.clone();

            if let MediaType::Video { path, .. } = &media_change.media_type {
                let effective_mute = mute || media_change.mute;

                if effective_mute || last_audio_path.as_deref() != Some(path.as_str()) {
                    if let Some(mut child) = last_audio_child.take() {
                        let _ = child.kill();
                        let _ = child.wait();
                        tracing::debug!(event = "audio_player_stopped", "Stopped ffplay");
                    }
                }

                if !effective_mute && last_audio_path.as_deref() != Some(path.as_str()) {
                    let audio_path = path.clone();
                    match std::process::Command::new("ffplay")
                        .args(&[
                            "-nodisp",
                            "-autoexit",
                            "-hide_banner",
                            "-loglevel",
                            "error",
                            "-loop",
                            "0",
                            &audio_path,
                        ])
                        .spawn()
                    {
                        Ok(child) => {
                            last_audio_child = Some(child);
                            last_audio_path = Some(path.clone());
                            tracing::info!(event = "audio_player_started", path = %audio_path, "Started ffplay for audio");
                        }
                        Err(e) => {
                            tracing::warn!(event = "audio_player_fail", error = %e, path = %audio_path, "Failed to start ffplay");
                        }
                    }
                } else if effective_mute {
                    last_audio_path = None;
                }
            } else {
                if let Some(mut child) = last_audio_child.take() {
                    let _ = child.kill();
                    let _ = child.wait();
                    tracing::debug!(
                        event = "audio_player_stopped",
                        "Stopped ffplay due to non-video media"
                    );
                }
                last_audio_path = None;
            }

            let target_monitors = media_change.monitors.as_deref();
            wayland_manager.monitor_manager.update_media(
                target_monitors,
                media_change.media_type,
                fps,
            )?;
        }

        let any_video_updated = wayland_manager
            .monitor_manager
            .render_all(fifo_reader.as_mut())?;

        frame_count += 1;

        let elapsed = utils::get_time_millis() - frame_start;
        let target_time = match &current_media_type {
            MediaType::Shader(_) => {
                if fps == 0 {
                    16
                } else {
                    base_frame_time
                }
            }
            MediaType::Video { .. } => {
                if fps == 0 {
                    if any_video_updated { 16 } else { 33 }
                } else {
                    base_frame_time
                }
            }
            MediaType::Image { .. } => {
                if any_video_updated {
                    base_frame_time
                } else {
                    base_frame_time * 2
                }
            }
        };

        if elapsed < target_time {
            utils::sleep_millis(target_time - elapsed);
        }

        if frame_count % 300 == 0 {
            let now = utils::get_time_millis();
            let actual_fps = 300000 / (now - last_fps_check + 1);
            if actual_fps < (fps as u64 * 80 / 100) && fps > 0 {
                tracing::warn!(
                    event = "performance_degradation",
                    expected_fps = fps,
                    actual_fps,
                    media_type = ?current_media_type,
                    "Performance below expected threshold"
                );
            }
            last_fps_check = now;
        }
    }
}

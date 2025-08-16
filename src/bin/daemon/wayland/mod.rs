use crate::utils;
use crate::wayland::fifo::FifoReader;
use crate::wayland::monitors::create_monitor_state;
use crate::wayland::state::AppState;
use anyhow::{Result, anyhow};
use khronos_egl as egl;
use std::collections::HashMap;
use std::process::Child;
use std::sync::mpsc::Receiver;
use wayland_client::{Connection, protocol::wl_output};

use crate::ipc::MediaChange;
use crate::media::MediaType;

mod fifo;
mod monitors;
mod renderer;
mod state;

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

    let conn = Connection::connect_to_env()?;
    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();
    let mut app_state = AppState::new();
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
    let egl_instance = egl::Instance::new(egl::Static);
    let mut monitor_states = HashMap::new();

    for output_info in app_state.outputs.values() {
        if let Some(name) = &output_info.name {
            let ms = create_monitor_state(
                output_info,
                compositor,
                layer_shell,
                layer_name,
                media_type.clone(),
                &egl_instance,
                &conn,
                &qh,
                fps,
            )?;
            monitor_states.insert(name.clone(), ms);
            app_state.total_surfaces += 1;
        }
    }

    event_queue.roundtrip(&mut app_state)?;
    while app_state.configured_count < app_state.total_surfaces {
        tracing::debug!(
            event = "waiting_layer_config",
            configured = app_state.configured_count,
            total = app_state.total_surfaces,
            "Awaiting layer surface configuration"
        );
        event_queue.blocking_dispatch(&mut app_state)?;
    }
    event_queue.roundtrip(&mut app_state)?;

    for ms in monitor_states.values_mut() {
        if let Some((width, height)) = app_state.layer_surface_configs.get(&ms.layer_surface_id) {
            tracing::info!(
                event = "monitor_configured",
                output = %ms.output_name,
                width, height,
                "Applying initial layer surface config"
            );
            ms.resize(*width, *height)?;
        } else {
            tracing::error!(
                event = "monitor_no_config",
                output = %ms.output_name,
                "No layer surface config found for monitor"
            );
        }
    }

    let mut has_video = matches!(media_type, MediaType::Video { .. });
    for ms in monitor_states.values() {
        if has_video {
            egl_instance.swap_interval(ms.egl_display, 1)?;
            tracing::debug!(
                event = "swap_interval_set",
                output = %ms.output_name,
                interval = 1,
                "Swap interval set for video playback"
            );
        } else {
            let interval = if fps == 0 { 1 } else { 0 };
            egl_instance.swap_interval(ms.egl_display, interval)?;
            tracing::debug!(
                event = "swap_interval_set",
                output = %ms.output_name,
                interval,
                "Swap interval set"
            );
        }
    }

    let mut fifo_reader = fifo_path.map(FifoReader::new).transpose()?;
    tracing::info!(
        event = "render_loop_start",
        monitors = monitor_states.len(),
        "Starting render loop"
    );

    let mut last_audio_path: Option<String> = None;
    let mut last_audio_child: Option<Child> = None;
    let mut frame_count = 0u64;
    let mut last_fps_check = utils::get_time_millis();

    let target_frame_time = if fps > 0 { 1000 / fps as u64 } else { 16 };

    loop {
        let frame_start = utils::get_time_millis();

        // Process pending events & configs
        event_queue.dispatch_pending(&mut app_state)?;
        for ms in monitor_states.values_mut() {
            if let Some((width, height)) = app_state.layer_surface_configs.get(&ms.layer_surface_id)
            {
                if !ms.configured || ms.current_width != *width || ms.current_height != *height {
                    if let Some(config_output) =
                        app_state.surface_to_output.get(&ms.layer_surface_id)
                    {
                        if config_output == &ms.output_name {
                            ms.resize(*width, *height)?;
                        }
                    }
                }
            }
        }

        // IPC: media change
        if let Ok(media_change) = ipc_receiver.try_recv() {
            let new_has_video = matches!(media_change.media_type, MediaType::Video { .. });
            if has_video != new_has_video {
                has_video = new_has_video;
                for ms in monitor_states.values() {
                    if has_video {
                        egl_instance.swap_interval(ms.egl_display, 1)?;
                    } else {
                        egl_instance.swap_interval(ms.egl_display, if fps == 0 { 1 } else { 0 })?;
                    }
                }
                tracing::info!(
                    event = "swap_interval_reconfigured",
                    has_video,
                    "Reconfigured swap intervals due to media type change"
                );
            }

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

            if let Some(target) = &media_change.monitor {
                if let Some(ms) = monitor_states.get_mut(target) {
                    egl_instance.make_current(
                        ms.egl_display,
                        Some(ms.egl_surface),
                        Some(ms.egl_surface),
                        Some(ms.egl_context),
                    )?;
                    tracing::info!(event = "media_update", output = %ms.output_name, "Updating media on single target");
                    ms.renderer.update_media(media_change.media_type, fps)?;
                }
            } else {
                tracing::info!(event = "media_update_all", "Updating media on all monitors");
                for ms in monitor_states.values_mut() {
                    egl_instance.make_current(
                        ms.egl_display,
                        Some(ms.egl_surface),
                        Some(ms.egl_surface),
                        Some(ms.egl_context),
                    )?;
                    ms.renderer
                        .update_media(media_change.media_type.clone(), fps)?;
                }
            }
        }

        // Render all outputs
        let mut any_video_updated = false;
        for ms in monitor_states.values_mut() {
            egl_instance.make_current(
                ms.egl_display,
                Some(ms.egl_surface),
                Some(ms.egl_surface),
                Some(ms.egl_context),
            )?;
            if ms.renderer.has_new_frame() {
                any_video_updated = true;
            }
            ms.renderer.draw(
                &mut fifo_reader,
                ms.current_width as i32,
                ms.current_height as i32,
                wl_output::Transform::Normal,
            )?;
            egl_instance.swap_buffers(ms.egl_display, ms.egl_surface)?;
        }

        frame_count += 1;

        // Frame pacing
        if has_video && fps == 0 {
            let elapsed = utils::get_time_millis() - frame_start;
            let min_frame_time = 8;
            if elapsed < min_frame_time {
                utils::sleep_millis(min_frame_time - elapsed);
            }
        } else if fps > 0 {
            let elapsed = utils::get_time_millis() - frame_start;
            if elapsed < target_frame_time {
                utils::sleep_millis(target_frame_time - elapsed);
            }
        } else {
            let adaptive_frame_time = if any_video_updated {
                target_frame_time
            } else {
                target_frame_time * 2
            };
            let elapsed = utils::get_time_millis() - frame_start;
            if elapsed < adaptive_frame_time {
                utils::sleep_millis(adaptive_frame_time - elapsed);
            }
        }

        if frame_count % 300 == 0 {

            let now = utils::get_time_millis();
            let _fps_actual = 300000 / (now - last_fps_check + 1);
            last_fps_check = now;

            tracing::debug!(
                event = "render_heartbeat",
                frames = frame_count,
                "Render loop heartbeat"
            );
        }
    }
}

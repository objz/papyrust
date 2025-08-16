use crate::utils;
use crate::wayland::fifo::FifoReader;
use crate::wayland::monitors::create_monitor_state;
use crate::wayland::state::AppState;
use anyhow::{Result, anyhow};
use khronos_egl as egl;
use log::info;
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
            )?;
            monitor_states.insert(name.clone(), ms);
            app_state.total_surfaces += 1;
        }
    }

    event_queue.roundtrip(&mut app_state)?;
    while app_state.configured_count < app_state.total_surfaces {
        event_queue.blocking_dispatch(&mut app_state)?;
    }
    event_queue.roundtrip(&mut app_state)?;

    // Apply configured sizes to monitor states - match by layer surface ID
    for ms in monitor_states.values_mut() {
        if let Some((width, height)) = app_state.layer_surface_configs.get(&ms.layer_surface_id) {
            eprintln!("Applying initial config to {}: {}x{}", ms.output_name, width, height);
            ms.resize(*width, *height)?;
        } else {
            eprintln!("No configuration found for monitor {}", ms.output_name);
        }
    }

    // Set proper swap intervals for video content
    let mut has_video = matches!(media_type, MediaType::Video { .. });
    for ms in monitor_states.values() {
        if has_video {
            // For video: use adaptive VSync (1) to prevent tearing but allow frame drops
            egl_instance.swap_interval(ms.egl_display, 1)?;
        } else {
            // For static content: use standard VSync based on FPS setting
            egl_instance.swap_interval(ms.egl_display, if fps == 0 { 1 } else { 0 })?;
        }
    }

    let mut fifo_reader = fifo_path.map(FifoReader::new).transpose()?;
    info!(
        "Starting render loop with {} monitors",
        monitor_states.len()
    );

    let mut last_audio_path: Option<String> = None;
    let mut last_audio_child: Option<Child> = None;
    let mut frame_count = 0u64;
    let mut last_fps_check = utils::get_time_millis();

    // Adaptive frame timing for video
    let target_frame_time = if fps > 0 { 1000 / fps as u64 } else { 16 }; // Default to ~60fps
    let mut adaptive_frame_time = target_frame_time;

    loop {
        let frame_start = utils::get_time_millis();

        // Handle configuration changes
        event_queue.dispatch_pending(&mut app_state)?;
        
        // Apply any new configuration sizes - match correctly by layer surface ID
        for ms in monitor_states.values_mut() {
            if let Some((width, height)) = app_state.layer_surface_configs.get(&ms.layer_surface_id) {
                if !ms.configured || ms.current_width != *width || ms.current_height != *height {
                    // Verify this config belongs to our surface
                    if let Some(config_output) = app_state.surface_to_output.get(&ms.layer_surface_id) {
                        if config_output == &ms.output_name {
                            ms.resize(*width, *height)?;
                        }
                    }
                }
            }
        }

        if let Ok(media_change) = ipc_receiver.try_recv() {
            let new_has_video = matches!(media_change.media_type, MediaType::Video { .. });
            
            // Update swap intervals if media type changed
            if has_video != new_has_video {
                has_video = new_has_video;
                for ms in monitor_states.values() {
                    if has_video {
                        egl_instance.swap_interval(ms.egl_display, 1)?;
                    } else {
                        egl_instance.swap_interval(ms.egl_display, if fps == 0 { 1 } else { 0 })?;
                    }
                }
            }

            if let MediaType::Video { path, .. } = &media_change.media_type {
                let effective_mute = mute || media_change.mute;

                if effective_mute || last_audio_path.as_deref() != Some(path.as_str()) {
                    if let Some(mut child) = last_audio_child.take() {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                }

                if !effective_mute && last_audio_path.as_deref() != Some(path.as_str()) {
                    let audio_path = path.clone();
                    if let Ok(child) = std::process::Command::new("ffplay")
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
                        last_audio_child = Some(child);
                        last_audio_path = Some(path.clone());
                    }
                } else if effective_mute {
                    last_audio_path = None;
                }
            } else {
                if let Some(mut child) = last_audio_child.take() {
                    let _ = child.kill();
                    let _ = child.wait();
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
                    ms.renderer.update_media(media_change.media_type)?;
                }
            } else {
                for ms in monitor_states.values_mut() {
                    egl_instance.make_current(
                        ms.egl_display,
                        Some(ms.egl_surface),
                        Some(ms.egl_surface),
                        Some(ms.egl_context),
                    )?;
                    ms.renderer.update_media(media_change.media_type.clone())?;
                }
            }
        }

        // Render to all monitors
        let mut any_video_updated = false;
        for ms in monitor_states.values_mut() {
            egl_instance.make_current(
                ms.egl_display,
                Some(ms.egl_surface),
                Some(ms.egl_surface),
                Some(ms.egl_context),
            )?;
            
            // Check if video frame actually updated
            let video_updated = ms.renderer.has_new_frame();
            if video_updated {
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

        // Adaptive frame timing for videos
        if has_video {
            if any_video_updated {
                // Video frame updated, use normal timing
                adaptive_frame_time = target_frame_time;
            } else {
                // No new video frame, reduce rendering frequency to save resources
                adaptive_frame_time = target_frame_time * 2;
            }
        }

        // Frame rate limiting with adaptive timing
        if fps > 0 || has_video {
            let elapsed = utils::get_time_millis() - frame_start;
            if elapsed < adaptive_frame_time {
                utils::sleep_millis(adaptive_frame_time - elapsed);
            }
        }

        // Debug frame rate every 5 seconds
        if frame_count % 300 == 0 {
            let now = utils::get_time_millis();
            let fps_actual = 300000 / (now - last_fps_check + 1);
            eprintln!("Actual FPS: {}, Video updated: {}", fps_actual, any_video_updated);
            last_fps_check = now;
        }
    }
}

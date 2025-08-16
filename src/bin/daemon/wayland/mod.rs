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
use wayland_client::Connection;

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

    for ms in monitor_states.values() {
        egl_instance.swap_interval(ms.egl_display, if fps == 0 { 1 } else { 0 })?;
    }

    let mut fifo_reader = fifo_path.map(FifoReader::new).transpose()?;
    info!(
        "Starting render loop with {} monitors",
        monitor_states.len()
    );

    let mut last_audio_path: Option<String> = None;
    let mut last_audio_child: Option<Child> = None;

    loop {
        let frame_start = utils::get_time_millis();

        if let Ok(media_change) = ipc_receiver.try_recv() {
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

        event_queue.dispatch_pending(&mut app_state)?;
        for ms in monitor_states.values_mut() {
            egl_instance.make_current(
                ms.egl_display,
                Some(ms.egl_surface),
                Some(ms.egl_surface),
                Some(ms.egl_context),
            )?;
            ms.renderer.draw(
                &mut fifo_reader,
                ms.output_info.width,
                ms.output_info.height,
                ms.output_info.transform,
            )?;
            egl_instance.swap_buffers(ms.egl_display, ms.egl_surface)?;
        }

        if fps > 0 {
            let elapsed = utils::get_time_millis() - frame_start;
            let target = 1000 / fps as u64;
            if elapsed < target {
                utils::sleep_millis(target - elapsed);
            }
        }
    }
}

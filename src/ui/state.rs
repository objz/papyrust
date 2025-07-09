use iced::{widget::image::Handle, Task};

use crate::{Message, Papyrust};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Discover,
    Library,
}

impl Default for Page {
    fn default() -> Self {
        Page::Library
    }
}

pub fn update(app: &mut Papyrust, message: Message) -> Task<Message> {
    match message {
        Message::SwitchPage(page) => {
            app.current_page = page;
            if page == Page::Library {
                return app.library.next().unwrap_or_else(Task::none);
            }
            Task::none()
        }
        Message::PreviewDecoded(idx, w, h, pixels) => {
            let handle = Handle::from_rgba(w, h, pixels);
            app.library.preview[idx] = Some(handle);
            app.library.next().unwrap_or_else(Task::none)
        }
        Message::PreviewError(_idx) => app.library.next().unwrap_or_else(Task::none),
        Message::Tick => {
            app.tick();
            Task::none()
        }
        Message::OpenPopup(project) => {
            app.popup_state = Some(project.clone());

            if let Some(file_name) = &project.meta.file {
                let video_path = format!("{}/{}", project.path, file_name);
                if app.should_load(&video_path) {
                    return Papyrust::load_video_async(video_path);
                }
            }
            Task::none()
        }
        Message::ClosePopup => {
            for video in app.videos.values_mut() {
                video.set_paused(true);
            }
            app.popup_state = None;
            Task::none()
        }
        Message::ApplyProject(project) => {
            if let Some(popup) = &app.popup_state {
                if popup.path == project.path {
                    app.popup_state = Some(project.clone());
                }
            } else {
                app.popup_state = Some(project.clone());
            }

            if let Some(file_name) = &project.meta.file {
                let video_path = format!("{}/{}", project.path, file_name);
                crate::ui::ipc::set_video("DP-3".to_string(), video_path, None).unwrap_or_else(
                    |e| {
                        eprintln!("Failed to set video: {}", e);
                    },
                );
            }
            Task::none()
        }
        Message::LoadVideo(path) => {
            app.load_video(&path);
            Task::none()
        }
        Message::VideoLoaded(path) => {
            app.load_video(&path);
            Task::none()
        }
        Message::VideoError(path, error) => {
            eprintln!("Failed to load video {}: {}", path, error);
            Task::none()
        }
        Message::DoNothing => Task::none(),
    }
}

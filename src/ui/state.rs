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
            if let Some(file_name) = &project.meta.file {
                let video_path = format!("{}/{}", project.path, file_name);
                if app.should_load(&video_path) {
                    app.load_video(&video_path);
                }
            }
            app.popup_state = Some(project);
            Task::none()
        }
        Message::ClosePopup => {
            app.popup_state = None;
            Task::none()
        }
        Message::LoadVideo(path) => {
            app.load_video(&path);
            Task::none()
        }
    }
}

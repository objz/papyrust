use iced::Task;

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
            Task::none()
        }
        Message::PreviewReady(index, handle_opt) => {
            if let Some(handle) = handle_opt {
                if index < app.library.preview.len() {
                    app.library.preview[index] = Some(handle);
                }
            }
            Task::none()
        }
    }
}

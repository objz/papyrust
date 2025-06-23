use std::fs;

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
            Task::none()
        }

        Message::NextProject => {
            app.library.load_next();
            let idx = app.library.projects.len() - 1;
            let mut tasks = Vec::new();

            if let Some(name) = app.library.projects[idx].meta.preview.as_ref() {
                let path = format!("{}/{}", app.library.projects[idx].path, name);
                tasks.push(Task::perform(
                    async move { (idx, fs::read(path).ok()) },
                    |(i, data)| Message::PreviewLoaded(i, data),
                ));
            }

            if app.library.remaining() {
                tasks.push(Task::perform(async {}, |_| Message::NextProject));
            }

            Task::batch(tasks)
        }

        Message::PreviewLoaded(index, bytes) => {
            if let Some(bytes) = bytes {
                let handle = Handle::from_bytes(bytes);
                if index < app.library.preview.len() {
                    app.library.preview[index] = Some(handle);
                }
            }
            Task::none()
        }
    }
}

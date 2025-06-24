use std::time::Duration;

use iced::{widget::image::Handle, Task};
use tokio::fs;

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
            app.library.load_project();
            let mut tasks = app.library.load_previews();

            if app.library.remaining() {
                tasks.push(Task::perform(
                    async move {
                        tokio::time::sleep(Duration::from_millis(16)).await;
                    },
                    |_| Message::NextProject,
                ));
            }

            Task::batch(tasks)
        }

        Message::PreviewLoaded(index, bytes) => {
            if let Some(bytes) = bytes {
                return Task::perform(
                    async move {
                        let handle = Handle::from_bytes(bytes);
                        (index, handle)
                    },
                    |(idx, handle)| Message::PreviewReady(idx, handle),
                );
            }
            Task::none()
        }
        Message::LoadPreview(index, path) => Task::perform(
            async move {
                match fs::read(&path).await {
                    Ok(bytes) => (index, Some(bytes)),
                    Err(_) => (index, None),
                }
            },
            |(idx, data)| Message::PreviewLoaded(idx, data),
        ),
        Message::PreviewReady(index, handle) => {
            if index < app.library.preview.len() {
                app.library.preview[index] = Some(handle);
            }
            Task::none()
        }
    }
}

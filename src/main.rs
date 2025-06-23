use iced::widget::image::Handle;
use iced::{Element, Task};
use std::fs;

mod library;
mod ui;

use ui::{library::Library, state::Page, view};

pub struct Papyrust {
    pub current_page: Page,
    pub library: Library,
}

#[derive(Debug, Clone)]
pub enum Message {
    SwitchPage(Page),
    /// (project_index, optional_bytes)
    PreviewLoaded(usize, Option<Vec<u8>>),
}

impl Papyrust {
    fn new() -> (Self, Task<Message>) {
        let library = Library::new();

        // Spawn a Task for each preview file
        let tasks: Vec<_> = library
            .projects
            .iter()
            .enumerate()
            .filter_map(|(i, project)| {
                project.meta.preview.as_ref().map(|preview_name| {
                    let preview_path = format!("{}/{}", project.path, preview_name);
                    Task::perform(
                        async move {
                            let data = fs::read(preview_path).ok();
                            (i, data)
                        },
                        |(idx, data)| Message::PreviewLoaded(idx, data),
                    )
                })
            })
            .collect();

        (
            Papyrust {
                current_page: Page::default(),
                library,
            },
            Task::batch(tasks),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SwitchPage(page) => {
                self.current_page = page;
                Task::none()
            }
            Message::PreviewLoaded(index, maybe_bytes) => {
                if let Some(bytes) = maybe_bytes {
                    // Vec<u8> already implements Into<Bytes>, so just pass it directly:
                    let handle = Handle::from_bytes(bytes);
                    if index < self.library.preview_handles.len() {
                        self.library.preview_handles[index] = Some(handle);
                    }
                }
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        view::build(self)
    }
}

fn main() -> iced::Result {
    iced::application("Papyrust", Papyrust::update, Papyrust::view)
        .theme(|_| iced::theme::Theme::GruvboxDark)
        .run_with(Papyrust::new)
}

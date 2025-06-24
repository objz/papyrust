use iced::widget::image::Handle;
use iced::Task;
use iced::{
    widget::{column, container, scrollable, text},
    Element, Length,
};
use tokio::fs;

use crate::library::{loader::Loader, project::Project};
use crate::{Message, Papyrust};

pub struct Library {
    loader: Loader,
    pub projects: Vec<Project>,
    pub preview: Vec<Option<Handle>>,
}

impl Library {
    pub fn new() -> Self {
        Self {
            loader: Loader::new(),
            projects: Vec::new(),
            preview: Vec::new(),
        }
    }

    pub fn load_project(&mut self) {
        if let Some(result) = self.loader.next() {
            match result {
                Ok(project) => {
                    self.projects.push(project);
                    self.preview.push(None);
                }
                Err(e) => eprintln!("Project parse error: {}", e),
            }
        }
    }

    pub fn load_previews(&mut self) -> Vec<Task<Message>> {
        self.projects
            .iter()
            .enumerate()
            .filter_map(|(idx, proj)| {
                if self.preview[idx].is_none() {
                    proj.meta.preview.as_ref().map(|name| {
                        let path = format!("{}/{}", proj.path, name);
                        Task::perform(
                            async move {
                                match fs::read(&path).await {
                                    Ok(bytes) => (idx, Some(bytes)),
                                    Err(_) => (idx, None),
                                }
                            },
                            |(i, data)| Message::PreviewLoaded(i, data),
                        )
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn remaining(&self) -> bool {
        self.loader.remaining() > 0
    }
}

pub fn build(app: &Papyrust) -> Element<Message> {
    let lib = &app.library;
    let grid = super::view::create_grid(&lib.projects, &lib.preview);

    container(scrollable(column![text("Library").size(30), grid]))
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

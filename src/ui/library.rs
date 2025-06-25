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
    pub projects: Vec<Project>,
    pub preview: Vec<Option<Handle>>,
}

impl Library {
    pub fn new() -> Self {
        let mut loader = Loader::new();
        let mut projects = Vec::new();
        let mut preview = Vec::new();

        while let Some(result) = loader.next() {
            match result {
                Ok(project) => {
                    projects.push(project);
                    preview.push(None);
                }
                Err(e) => eprintln!("Project parse error: {}", e),
            }
        }

        Self { projects, preview }
    }

    pub fn next(&mut self) -> Option<Task<Message>> {
        self.projects
            .iter()
            .enumerate()
            .find(|(idx, proj)| self.preview[*idx].is_none() && proj.meta.preview.is_some())
            .map(|(idx, proj)| {
                let name = proj.meta.preview.as_ref().unwrap().clone();
                let path = format!("{}/{}", proj.path, name);
                Task::perform(
                    async move {
                        let data = tokio::fs::read(&path).await.ok();
                        (idx, data.map(Handle::from_bytes))
                    },
                    |(i, handle)| Message::PreviewReady(i, handle),
                )
            })
    }

    pub fn load_previews(&self) -> Vec<Task<Message>> {
        self.projects
            .iter()
            .enumerate()
            .filter_map(|(idx, proj)| {
                proj.meta.preview.as_ref().map(|name| {
                    let path = format!("{}/{}", proj.path, name);
                    Task::perform(
                        async move {
                            let data = fs::read(&path).await.ok();
                            (idx, data.map(Handle::from_bytes))
                        },
                        |(i, handle)| Message::PreviewReady(i, handle),
                    )
                })
            })
            .collect()
    }

    pub fn remaining(&self) -> bool {
        !self.projects.is_empty()
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

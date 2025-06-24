use iced::widget::image::Handle;
use iced::{
    widget::{column, container, scrollable, text},
    Element, Length,
};

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
                    // self.preview.push(None);
                }
                Err(e) => eprintln!("Project parse error: {}", e),
            }
        }
    }

    pub fn load_preview(&mut self) {
        if let Some(project) = self.projects.last() {
            if let Some(name) = &project.meta.preview {
                let path = format!("{}/{}", project.path, name);
                let handle = Handle::from_path(path);
                self.preview.push(Some(handle));
            } else {
                self.preview.push(None);
            }
        } else {
            self.preview.push(None);
        }
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

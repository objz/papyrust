use iced::widget::image::Handle;
use iced::Task;
use iced::{
    widget::{column, container, scrollable, text},
    Element, Length,
};
use image::load_from_memory;
use tokio::{fs, task};

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
                        let buf = fs::read(&path).await.ok();
                        if let Some(bytes) = buf {
                            let decode = task::spawn_blocking(move || {
                                let img = load_from_memory(&bytes).ok()?;
                                let rgba = img.to_rgba8();
                                let (w, h) = rgba.dimensions();
                                Some((w, h, rgba.into_raw()))
                            })
                            .await
                            .ok()
                            .flatten();

                            if let Some((w, h, pixels)) = decode {
                                return (idx, Ok((w, h, pixels)));
                            }
                        }
                        (idx, Err(()))
                    },
                    |(i, result)| match result {
                        Ok((w, h, pixels)) => Message::PreviewDecoded(i, w, h, pixels),
                        Err(_) => Message::PreviewError(i),
                    },
                )
            })
    }
}

pub fn build(app: &Papyrust) -> Element<Message> {
    let lib = &app.library;
    let grid = super::view::create_grid(&app, &lib.projects, &lib.preview);

    container(scrollable(column![text("Library").size(30), grid]))
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

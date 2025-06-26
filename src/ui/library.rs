use iced::alignment::{Horizontal, Vertical};
use iced::widget::image::Handle;
use iced::widget::{Column, Container};
use iced::{
    widget::{column, container, scrollable, text},
    Element, Length,
};
use iced::{Alignment, Task};
use image::load_from_memory;
use tokio::{fs, task};

use crate::library::{loader::Loader, project::Project};
use crate::{Message, Papyrust};

pub struct Library {
    pub projects: Vec<Project>,
    pub preview: Vec<Option<Handle>>,
}

const PREVIEW_WIDTH: f32 = 140.0;
const PREVIEW_HEIGHT: f32 = 140.0;

const ITEM_WIDTH: f32 = 160.0;
const ITEM_HEIGHT: f32 = 200.0;

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

pub fn render_item<'a>(
    app: &Papyrust,
    project: &'a Project,
    preview: Option<Handle>,
) -> Element<'a, Message> {
    let title = project.meta.title.as_deref().unwrap_or("Untitled");
    let preview = create_preview(app, preview, project);

    Container::new(
        Column::new()
            .align_x(Alignment::Center)
            .push(preview)
            .push(
                text(title)
                    .size(14)
                    .width(Length::Fixed(ITEM_WIDTH - 20.0))
                    .align_x(Alignment::Center),
            )
            .spacing(8)
            .padding(10),
    )
    .width(Length::Fixed(ITEM_WIDTH))
    .height(Length::Fixed(ITEM_HEIGHT))
    .style(|_theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            0.0, 0.0, 0.0, 0.05,
        ))),
        border: iced::Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

fn create_preview<'a>(
    app: &Papyrust,
    preview: Option<Handle>,
    project: &'a Project,
) -> Element<'a, Message> {
    if let Some(handle) = preview {
        Container::new(
            iced::widget::image(handle)
                .width(Length::Fixed(PREVIEW_WIDTH))
                .height(Length::Fixed(PREVIEW_HEIGHT)),
        )
        .width(Length::Fixed(PREVIEW_WIDTH))
        .height(Length::Fixed(PREVIEW_HEIGHT))
        .clip(true)
        .style(|_theme| iced::widget::container::Style {
            border: iced::Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
    } else if project.meta.preview.is_some() {
        let dots = match app.animation_state {
            0 => "Loading.  ",
            1 => "Loading.. ",
            2 => "Loading...",
            _ => "Loading   ",
        };

        Container::new(text(dots))
            .width(Length::Fixed(PREVIEW_WIDTH))
            .height(Length::Fixed(PREVIEW_HEIGHT))
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
            .style(|_theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    0.5, 0.5, 0.5, 0.1,
                ))),
                border: iced::Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    } else {
        Container::new(text("No preview"))
            .width(Length::Fixed(PREVIEW_WIDTH))
            .height(Length::Fixed(PREVIEW_HEIGHT))
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
            .style(|_theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    0.5, 0.5, 0.5, 0.1,
                ))),
                border: iced::Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    }
}

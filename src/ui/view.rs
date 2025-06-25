use iced::Alignment;
use iced::{
    alignment::{Horizontal, Vertical},
    widget::{image, image::Handle, text, Column, Container},
    Element, Length, Padding,
};
use iced_aw::Wrap;

use crate::{library::project::Project, Message, Papyrust};

use super::{discover, library, panel, state};

const PREVIEW_WIDTH: f32 = 140.0;
const PREVIEW_HEIGHT: f32 = 140.0;

const ITEM_WIDTH: f32 = 160.0;
const ITEM_HEIGHT: f32 = 180.0;

pub fn build(app: &Papyrust) -> Element<Message> {
    let content = match app.current_page {
        state::Page::Discover => discover::build(app),
        state::Page::Library => library::build(app),
    };

    let main = Column::new()
        .push(content)
        .width(Length::Fill)
        .height(Length::Fill);

    let panel = Container::new(panel::build(app))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
            top: 0.0,
            right: 20.0,
            bottom: 0.0,
            left: 0.0,
        })
        .align_x(Horizontal::Center)
        .align_y(Vertical::Bottom);

    Column::new()
        .push(main)
        .push(
            Container::new(panel)
                .width(Length::Fill)
                .height(Length::Fixed(80.0)),
        )
        .into()
}

pub fn create_grid<'a>(
    projects: &'a [Project],
    preview: &'a [Option<Handle>],
) -> Element<'a, Message> {
    let mut items = Vec::new();

    for (idx, project) in projects.iter().enumerate() {
        let handle = preview.get(idx).and_then(Clone::clone);
        items.push(render_item(project, handle));
    }

    Container::new(Wrap::with_elements(items).spacing(8.0).line_spacing(8.0))
        .width(Length::Fill)
        .padding(8)
        .into()
}

fn render_item<'a>(project: &'a Project, preview: Option<Handle>) -> Element<'a, Message> {
    let title = project.meta.title.as_deref().unwrap_or("Untitled");
    let preview = create_preview(preview, project);

    Container::new(
        Column::new()
            .align_x(Alignment::Center)
            .push(preview)
            .push(
                text(title)
                    .size(16)
                    .width(Length::Fixed(ITEM_WIDTH - 20.0))
                    .height(Length::Fixed(ITEM_HEIGHT - 120.0))
                    .align_x(Alignment::Center),
            )
            .spacing(5)
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

fn create_preview<'a>(preview: Option<Handle>, project: &'a Project) -> Element<'a, Message> {
    if let Some(handle) = preview {
        Container::new(
            image(handle)
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
        Container::new(text("Loading..."))
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

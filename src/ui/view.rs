use iced::widget::container;
use iced::{
    alignment::{Horizontal, Vertical},
    widget::{image, image::Handle, text, Column, Container, Row},
    Element, Length, Padding,
};

use crate::{library::project::Project, Message, Papyrust};

use super::{discover, library, panel, state};

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
    const ITEMS_PER_ROW: usize = 3;
    let mut rows = Vec::new();
    let mut idx = 0;

    for chunk in projects.chunks(ITEMS_PER_ROW) {
        let mut cells = Vec::new();

        for project in chunk {
            let handle = preview.get(idx).and_then(Clone::clone);
            cells.push(render_item(project, handle));
            idx += 1;
        }

        while cells.len() < ITEMS_PER_ROW {
            cells.push(container(text("")).width(Length::FillPortion(1)).into());
        }

        rows.push(
            Row::with_children(cells)
                .spacing(15)
                .width(Length::Fill)
                .into(),
        );
    }

    Column::with_children(rows)
        .spacing(15)
        .width(Length::Fill)
        .into()
}

fn render_item<'a>(project: &'a Project, preview: Option<Handle>) -> Element<'a, Message> {
    let title = project.meta.title.as_deref().unwrap_or("Untitled");
    let preview = create_preview(preview, project);

    container(
        Column::new()
            .push(preview)
            .push(text(title).size(16))
            .spacing(5)
            .padding(10),
    )
    .width(Length::FillPortion(1))
    .height(Length::Fixed(150.0))
    .into()
}

fn create_preview<'a>(preview: Option<Handle>, project: &'a Project) -> Element<'a, Message> {
    if let Some(handle) = preview {
        image(handle)
            .width(Length::Fill)
            .height(Length::Fixed(100.0))
            .into()
    } else if project.meta.preview.is_some() {
        container(text("Loading..."))
            .width(Length::Fill)
            .height(Length::Fixed(100.0))
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
            .into()
    } else {
        container(text("No preview"))
            .width(Length::Fill)
            .height(Length::Fixed(100.0))
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
            .into()
    }
}

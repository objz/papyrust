use iced::widget::image::Handle;
use iced::{
    alignment::{Horizontal, Vertical},
    widget::{column, container, image, scrollable, text, Column, Row},
    Element, Length,
};

use crate::{
    library::{loader::discover_projects, project::Project},
    Message, Papyrust,
};

pub struct Library {
    pub projects: Vec<Project>,
    pub preview_handles: Vec<Option<Handle>>,
}

impl Library {
    pub fn new() -> Self {
        let projects = discover_projects();
        let preview_handles = vec![None; projects.len()];
        Self {
            projects,
            preview_handles,
        }
    }
}

pub fn build(app: &Papyrust) -> Element<Message> {
    let lib = &app.library;
    let grid = create_grid(&lib.projects, &lib.preview_handles);

    container(scrollable(column!(text("Library").size(30), grid)))
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn create_grid<'a>(
    projects: &'a [Project],
    preview_handles: &'a [Option<Handle>],
) -> Element<'a, Message> {
    const ITEMS_PER_ROW: usize = 3;
    let mut rows = Vec::new();
    let mut idx = 0;

    for chunk in projects.chunks(ITEMS_PER_ROW) {
        let mut cells = Vec::new();
        for project in chunk {
            let handle = preview_handles.get(idx).and_then(|h| h.clone());
            cells.push(render_item(project, handle));
            idx += 1;
        }
        // pad out last row
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

fn render_item<'a>(project: &'a Project, preview_handle: Option<Handle>) -> Element<'a, Message> {
    let title = project.meta.title.as_deref().unwrap_or("Untitled");
    let preview = create_preview(preview_handle, project);

    container(
        column![preview, text(title).size(16)]
            .spacing(5)
            .padding(10),
    )
    .width(Length::FillPortion(1))
    .height(Length::Fixed(150.0))
    .into()
}

fn create_preview<'a>(
    preview_handle: Option<Handle>,
    project: &'a Project,
) -> Element<'a, Message> {
    if let Some(handle) = preview_handle {
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

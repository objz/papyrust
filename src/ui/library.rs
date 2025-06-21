use iced::{
    widget::{column, container, scrollable, text, Column, Row},
    Element, Length,
};

use crate::{
    library::{loader::discover_projects, project::Project},
    Message, Papyrust,
};

pub struct Library {
    projects: Vec<Project>,
}

impl Library {
    pub fn new() -> Self {
        let projects = discover_projects();
        Self { projects }
    }
}

pub fn build(app: &Papyrust) -> Element<Message> {
    let library = &app.library;

    let grid = create_grid(&library.projects);

    container(scrollable(column!(text("Library").size(30), grid)))
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn create_grid(projects: &[Project]) -> Element<Message> {
    const ITEMS_PER_ROW: usize = 3;

    let mut rows = Vec::new();

    for chunk in projects.chunks(ITEMS_PER_ROW) {
        let mut row_items = Vec::new();

        for project in chunk {
            row_items.push(render_item(project));
        }

        while row_items.len() < ITEMS_PER_ROW {
            row_items.push(container(text("")).width(Length::FillPortion(1)).into());
        }

        let project_row = Row::with_children(row_items)
            .spacing(15)
            .width(Length::Fill);

        rows.push(project_row.into());
    }

    Column::with_children(rows)
        .spacing(15)
        .width(Length::Fill)
        .into()
}

fn render_item(project: &Project) -> Element<Message> {
    let title = project.meta.title.as_deref().unwrap_or("Untitled");

    let file_type = if let Some(file_type) = &project.meta.file_type {
        format!("Type: {:?}", file_type)
    } else {
        "Type: Unknown".to_string()
    };

    container(
        column![
            text(title).size(16),
            text(file_type).size(10),
            text(format!(
                "Path: {}/{}",
                project.path,
                project.meta.preview.as_deref().unwrap_or("no_preview")
            ))
            .size(8),
        ]
        .spacing(5)
        .padding(10),
    )
    .width(Length::FillPortion(1))
    .height(Length::Fixed(150.0))
    .into()
}

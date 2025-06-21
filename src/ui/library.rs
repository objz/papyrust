use std::collections::HashMap;
use std::path::Path;

use iced::{
    widget::{column, container, image, scrollable, text, Column, Row},
    Element, Length,
};
use iced_video_player::{Video, VideoPlayer};

use crate::{
    library::{loader::discover_projects, project::Project},
    Message, Papyrust,
};

pub struct Library {
    projects: Vec<Project>,
    videos: HashMap<String, Video>,
}

impl Library {
    pub fn new() -> Self {
        let projects = discover_projects();
        let mut videos = HashMap::new();

        for project in &projects {
            if let Some(preview_name) = &project.meta.preview {
                let preview_path = format!("{}/{}", project.path, preview_name);
                let path = Path::new(&preview_path);

                if path.exists() {
                    if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
                        if ext == "gif" {
                            if let Ok(url) = url::Url::from_file_path(&preview_path) {
                                if let Ok(video) = Video::new(&url) {
                                    videos.insert(preview_path.clone(), video);
                                }
                            }
                        }
                    }
                }
            }
        }

        Self { projects, videos }
    }
}

pub fn build(app: &Papyrust) -> Element<Message> {
    let library = &app.library;

    let grid = create_grid(&library.projects, &library.videos);

    container(scrollable(column!(text("Library").size(30), grid)))
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn create_grid<'a>(
    projects: &'a [Project],
    videos: &'a HashMap<String, Video>,
) -> Element<'a, Message> {
    const ITEMS_PER_ROW: usize = 3;

    let mut rows = Vec::new();

    for chunk in projects.chunks(ITEMS_PER_ROW) {
        let mut row_items = Vec::new();

        for project in chunk {
            row_items.push(render_item(project, videos));
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

fn render_item<'a>(
    project: &'a Project,
    videos: &'a HashMap<String, Video>,
) -> Element<'a, Message> {
    let title = project.meta.title.as_deref().unwrap_or("Untitled");

    let _file_type = if let Some(file_type) = &project.meta.file_type {
        format!("Type: {:?}", file_type)
    } else {
        "Type: Unknown".to_string()
    };

    let preview = create_preview(project, videos);

    container(
        column![preview, text(title).size(16).size(8),]
            .spacing(5)
            .padding(10),
    )
    .width(Length::FillPortion(1))
    .height(Length::Fixed(150.0))
    .into()
}

fn create_preview<'a>(
    project: &'a Project,
    videos: &'a HashMap<String, Video>,
) -> Element<'a, Message> {
    if let Some(preview_name) = &project.meta.preview {
        let preview_path = format!("{}/{}", project.path, preview_name);
        let path = Path::new(&preview_path);

        if path.exists() {
            match path.extension().and_then(|ext| ext.to_str()) {
                Some("jpg") | Some("jpeg") | Some("png") => image(preview_path)
                    .width(Length::Fill)
                    .height(Length::Fixed(100.0))
                    .into(),
                Some("gif") => {
                    if let Some(video) = videos.get(&preview_path) {
                        VideoPlayer::new(video)
                            .width(Length::Fill)
                            .height(Length::Fixed(100.0))
                            .content_fit(iced::ContentFit::Contain)
                            .into()
                    } else {
                        container(text("Video unavailable"))
                            .width(Length::Fill)
                            .height(Length::Fixed(100.0))
                            .into()
                    }
                }
                _ => container(text("Unsupported preview"))
                    .width(Length::Fill)
                    .height(Length::Fixed(100.0))
                    .into(),
            }
        } else {
            container(text("No preview"))
                .width(Length::Fill)
                .height(Length::Fixed(100.0))
                .into()
        }
    } else {
        container(text("No preview"))
            .width(Length::Fill)
            .height(Length::Fixed(100.0))
            .into()
    }
}

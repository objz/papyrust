use iced::alignment::{Horizontal, Vertical};
use iced::widget::image::Handle;
use iced::widget::{Button, Column, Container};
use iced::{
    widget::{column, container, scrollable, text},
    Element, Length,
};
use iced::{Alignment, Padding, Task};
use image::{imageops, load_from_memory, RgbaImage};
use tokio::{fs, task};

use crate::ui::loader::project::{Project, ProjectType};
use crate::ui::loader::Loader;
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
                    // Skip for now all non-video projects
                    if project.meta.file_type != Some(ProjectType::Video) {
                        continue;
                    }
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
                                let mut rgba = img.to_rgba8();

                                // Ik this is not the most efficient way to handle this, but iced forces me to do it this way
                                rgba = Self::resize_image(rgba, PREVIEW_WIDTH as u32);
                                rgba = Self::round_image(rgba, 4.0);
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

    fn round_image(img: RgbaImage, radius: f32) -> RgbaImage {
        let (width, height) = img.dimensions();
        let mut rounded = img.clone();

        let radius_u32 = radius as u32;
        let right_bound = width.saturating_sub(radius_u32);
        let bottom_bound = height.saturating_sub(radius_u32);
        let radius_sq = radius * radius;

        let corners = [
            (0..radius_u32, 0..radius_u32),
            (right_bound..width, 0..radius_u32),
            (0..radius_u32, bottom_bound..height),
            (right_bound..width, bottom_bound..height),
        ];

        for (x_range, y_range) in corners {
            for y in y_range {
                for x in x_range.clone() {
                    let (dx, dy) = match (x < radius_u32, y < radius_u32) {
                        (true, true) => (radius - x as f32, radius - y as f32),
                        (false, true) => {
                            (x as f32 - (width as f32 - radius - 1.0), radius - y as f32)
                        }
                        (true, false) => {
                            (radius - x as f32, y as f32 - (height as f32 - radius - 1.0))
                        }
                        (false, false) => (
                            x as f32 - (width as f32 - radius - 1.0),
                            y as f32 - (height as f32 - radius - 1.0),
                        ),
                    };

                    if dx * dx + dy * dy > radius_sq {
                        rounded.get_pixel_mut(x, y)[3] = 0;
                    }
                }
            }
        }

        rounded
    }
    fn resize_image(img: RgbaImage, target_size: u32) -> RgbaImage {
        let (width, height) = img.dimensions();

        let scale = target_size as f32 / width.max(height) as f32;
        let new_width = (width as f32 * scale) as u32;
        let new_height = (height as f32 * scale) as u32;

        let src_image = fast_image_resize::images::Image::from_vec_u8(
            width,
            height,
            img.into_raw(),
            fast_image_resize::PixelType::U8x4,
        )
        .unwrap();

        let mut dst_image = fast_image_resize::images::Image::new(
            new_width,
            new_height,
            fast_image_resize::PixelType::U8x4,
        );

        let mut resizer = fast_image_resize::Resizer::new();

        let resize_options = fast_image_resize::ResizeOptions::new().resize_alg(
            fast_image_resize::ResizeAlg::Convolution(fast_image_resize::FilterType::Lanczos3),
        );

        resizer
            .resize(&src_image, &mut dst_image, Some(&resize_options))
            .unwrap();

        let resized_rgba =
            RgbaImage::from_raw(new_width, new_height, dst_image.into_vec()).unwrap();

        let crop_x = (new_width.saturating_sub(target_size)) / 2;
        let crop_y = (new_height.saturating_sub(target_size)) / 2;

        imageops::crop_imm(&resized_rgba, crop_x, crop_y, target_size, target_size).to_image()
    }
}

pub fn build(app: &Papyrust) -> Element<Message> {
    let lib = &app.library;
    let grid = crate::ui::view::create_grid(&app, &lib.projects, &lib.preview);

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

    Button::new(
        Container::new(
            Column::new()
                .align_x(Alignment::Center)
                .padding(Padding::new(0.0).top(4.0))
                .push(preview)
                .push(
                    text(title)
                        .size(14)
                        .style(|_theme: &_| iced::widget::text::Style {
                            color: Some(iced::Color::WHITE),
                            ..Default::default()
                        })
                        .width(Length::Fixed(ITEM_WIDTH - 20.0))
                        .align_x(Alignment::Center),
                )
                .spacing(8),
        )
        .width(Length::Fixed(ITEM_WIDTH))
        .height(Length::Fixed(ITEM_HEIGHT)),
    )
    .width(Length::Fixed(ITEM_WIDTH))
    .height(Length::Fixed(ITEM_HEIGHT))
    .style(|_theme, status| {
        let base_color = iced::Color::from_rgba(0.0, 0.0, 0.0, 0.15);
        let hover_color = iced::Color::from_rgba(0.5, 0.5, 0.5, 0.3);
        let border_color = iced::Color::from_rgba(0.0, 0.0, 0.0, 0.2);

        match status {
            iced::widget::button::Status::Hovered => iced::widget::button::Style {
                background: Some(iced::Background::Color(hover_color)),
                border: iced::Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: border_color,
                },
                shadow: iced::Shadow {
                    color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.1),
                    offset: iced::Vector::new(0.0, 2.0),
                    blur_radius: 4.0,
                },
                ..Default::default()
            },
            _ => iced::widget::button::Style {
                background: Some(iced::Background::Color(base_color)),
                border: iced::Border {
                    radius: 8.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            },
        }
    })
    .on_press(Message::OpenPopup(project.clone()))
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
        .into()
    } else if project.meta.preview.is_some() {
        let dots = match app.animation_state {
            0 => "Loading.  ",
            1 => "Loading.. ",
            2 => "Loading...",
            _ => "Loading   ",
        };

        Container::new(text(dots).style(|_theme: &_| iced::widget::text::Style {
            color: Some(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.6)),
            ..Default::default()
        }))
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
        Container::new(
            text("No preview").style(|_theme: &_| iced::widget::text::Style {
                color: Some(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.6)),
                ..Default::default()
            }),
        )
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

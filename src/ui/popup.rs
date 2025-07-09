use iced::{
    alignment::{Horizontal, Vertical},
    widget::{button, mouse_area, text, Button, Column, Container, Row, Space},
    Background, Border, Color, Element, Length, Padding, Shadow, Vector,
};
use iced_video_player::VideoPlayer;

use crate::{ui::loader::project::Project, Message, Papyrust};

pub fn build<'a>(app: &'a Papyrust, project: &'a Project) -> Element<'a, Message> {
    let title = project.meta.title.as_deref().unwrap_or("Untitled");

    let video_preview = create_preview(app, project);

    let close_button = Button::new(text("Close").size(16))
        .on_press(Message::ClosePopup)
        .padding(Padding::from([8, 12]))
        .style(|_theme, status| {
            let base = Color::from_rgba(0.2, 0.2, 0.2, 0.8);
            let hover = Color::from_rgba(0.3, 0.3, 0.3, 0.9);
            let border_color = Color::from_rgba(0.6, 0.6, 0.6, 0.5);

            button::Style {
                background: Some(Background::Color(
                    if matches!(status, iced::widget::button::Status::Hovered) {
                        hover
                    } else {
                        base
                    },
                )),
                border: Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: border_color,
                },
                text_color: Color::WHITE,
                ..Default::default()
            }
        });

    let apply_button = Button::new(text("Apply").size(16))
        .padding(Padding::from([8, 12]))
        .style(|_theme, status| {
            let base = Color::from_rgba(0.2, 0.2, 0.2, 0.8);
            let hover = Color::from_rgba(0.3, 0.3, 0.3, 0.9);
            let border_color = Color::from_rgba(0.6, 0.6, 0.6, 0.5);

            button::Style {
                background: Some(Background::Color(
                    if matches!(status, iced::widget::button::Status::Hovered) {
                        hover
                    } else {
                        base
                    },
                )),
                border: Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: border_color,
                },
                text_color: Color::WHITE,
                ..Default::default()
            }
        });

    let header = Row::new()
        .push(
            text(title)
                .size(24)
                .style(|_theme| iced::widget::text::Style {
                    color: Some(Color::WHITE),
                    ..Default::default()
                }),
        )
        .align_y(Vertical::Center);

    let footer_row = Row::new().spacing(10).push(close_button).push(apply_button);

    let footer = Container::new(footer_row)
        .align_x(Horizontal::Center)
        .width(Length::Fill);

    let popup_content = Column::new()
        .push(header)
        .push(video_preview)
        .push(Space::new(Length::Fill, Length::Fill))
        .push(footer)
        .spacing(20)
        .padding(20)
        .width(Length::Fill)
        .align_x(Horizontal::Center);

    let popup = Container::new(popup_content)
        .width(Length::Fixed(800.0))
        .height(Length::Fixed(600.0))
        .style(|_theme| iced::widget::container::Style {
            background: Some(Background::Color(Color::from_rgba(0.05, 0.05, 0.05, 0.98))),
            border: Border {
                radius: 16.0.into(),
                width: 2.0,
                color: Color::from_rgba(0.6, 0.6, 0.6, 0.4),
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                offset: Vector::new(0.0, 8.0),
                blur_radius: 24.0,
            },
            ..Default::default()
        })
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center);

    let protected = mouse_area(popup).on_press(Message::DoNothing);

    mouse_area(
        Container::new(protected)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_theme| iced::widget::container::Style {
                background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.85))),
                ..Default::default()
            })
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center),
    )
    .on_press(Message::ClosePopup)
    .into()
}

fn create_preview<'a>(app: &'a Papyrust, project: &'a Project) -> Element<'a, Message> {
    if let Some(file_name) = &project.meta.file {
        let video_path = format!("{}/{}", project.path, file_name);

        if let Some(video) = app.peek_video(&video_path) {
            let video_width = 720.0;
            let video_height = 405.0;

            Container::new(
                VideoPlayer::new(video)
                    .width(Length::Fixed(video_width))
                    .height(Length::Fixed(video_height)),
            )
            .width(Length::Fixed(video_width))
            .height(Length::Fixed(video_height))
            .style(|_theme| iced::widget::container::Style {
                background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.95))),
                border: Border {
                    radius: 12.0.into(),
                    width: 2.0,
                    color: Color::from_rgba(0.4, 0.4, 0.4, 0.5),
                },
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                    offset: Vector::new(0.0, 4.0),
                    blur_radius: 12.0,
                },
                ..Default::default()
            })
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
            .into()
        } else {
            let dots = match app.animation_state {
                0 => "Loading video.  ",
                1 => "Loading video.. ",
                2 => "Loading video...",
                _ => "Loading video   ",
            };

            let video_width = 720.0;
            let video_height = 405.0;

            Container::new(
                text(dots)
                    .size(18)
                    .style(|_theme| iced::widget::text::Style {
                        color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.8)),
                        ..Default::default()
                    }),
            )
            .width(Length::Fixed(video_width))
            .height(Length::Fixed(video_height))
            .style(|_theme| iced::widget::container::Style {
                background: Some(Background::Color(Color::from_rgba(0.15, 0.15, 0.15, 0.9))),
                border: Border {
                    radius: 12.0.into(),
                    width: 2.0,
                    color: Color::from_rgba(0.4, 0.4, 0.4, 0.5),
                },
                ..Default::default()
            })
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
            .into()
        }
    } else {
        let video_width = 720.0;
        let video_height = 405.0;

        Container::new(text("No video available").size(18).style(|_theme| {
            iced::widget::text::Style {
                color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.8)),
                ..Default::default()
            }
        }))
        .width(Length::Fixed(video_width))
        .height(Length::Fixed(video_height))
        .style(|_theme| iced::widget::container::Style {
            background: Some(Background::Color(Color::from_rgba(0.15, 0.15, 0.15, 0.9))),
            border: Border {
                radius: 12.0.into(),
                width: 2.0,
                color: Color::from_rgba(0.4, 0.4, 0.4, 0.5),
            },
            ..Default::default()
        })
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .into()
    }
}

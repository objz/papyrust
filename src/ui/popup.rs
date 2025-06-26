use iced::{
    alignment::{Horizontal, Vertical},
    widget::{button, text, Button, Column, Container, Row},
    Color, Element, Length, Padding,
};
use iced_video_player::VideoPlayer;

use crate::{library::project::Project, Message, Papyrust};

pub fn build<'a>(app: &'a Papyrust, project: &'a Project) -> Element<'a, Message> {
    let title = project.meta.title.as_deref().unwrap_or("Untitled");

    let video_preview = create_preview(app, project);

    let close_button = Button::new(text("Close").size(16))
        .on_press(Message::ClosePopup)
        .padding(Padding::from([8, 12]))
        .style(|_theme, status| {
            let base_color = Color::from_rgba(0.8, 0.2, 0.2, 0.8);
            let hover_color = Color::from_rgba(1.0, 0.3, 0.3, 0.9);

            button::Style {
                background: Some(iced::Background::Color(
                    if matches!(status, iced::widget::button::Status::Hovered) {
                        hover_color
                    } else {
                        base_color
                    },
                )),
                border: iced::Border {
                    radius: 20.0.into(),
                    width: 1.0,
                    color: Color::from_rgba(1.0, 1.0, 1.0, 0.3),
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
        .push(close_button)
        .align_y(Vertical::Center);

    let popup_content = Column::new()
        .push(header)
        .push(video_preview)
        .spacing(20)
        .align_x(iced::Alignment::Center);

    let popup = Container::new(popup_content)
        .width(Length::Fixed(800.0))
        .height(Length::Fixed(600.0))
        .padding(30)
        .style(|_theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(Color::from_rgba(
                0.05, 0.05, 0.05, 0.98,
            ))),
            border: iced::Border {
                radius: 16.0.into(),
                width: 2.0,
                color: Color::from_rgba(0.6, 0.6, 0.6, 0.4),
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                offset: iced::Vector::new(0.0, 8.0),
                blur_radius: 24.0,
            },
            ..Default::default()
        })
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center);

    Container::new(popup)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(Color::from_rgba(
                0.0, 0.0, 0.0, 0.85,
            ))),
            ..Default::default()
        })
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
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
                background: Some(iced::Background::Color(Color::from_rgba(
                    0.0, 0.0, 0.0, 0.95,
                ))),
                border: iced::Border {
                    radius: 12.0.into(),
                    width: 2.0,
                    color: Color::from_rgba(0.4, 0.4, 0.4, 0.5),
                },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                    offset: iced::Vector::new(0.0, 4.0),
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
                background: Some(iced::Background::Color(Color::from_rgba(
                    0.15, 0.15, 0.15, 0.9,
                ))),
                border: iced::Border {
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
            background: Some(iced::Background::Color(Color::from_rgba(
                0.15, 0.15, 0.15, 0.9,
            ))),
            border: iced::Border {
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

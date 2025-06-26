use iced::{
    alignment::{Horizontal, Vertical},
    widget::{button, text, Button, Column, Container},
    Color, Element, Length, Padding,
};
use iced_video_player::VideoPlayer;

use crate::{library::project::Project, Message, Papyrust};

pub fn build<'a>(app: &'a Papyrust, project: &'a Project) -> Element<'a, Message> {
    let title = project.meta.title.as_deref().unwrap_or("Untitled");

    // Create video preview widget
    let video_preview = create_preview(app, project);

    let popup_content = Column::new()
        .push(
            text(title)
                .size(24)
                .style(|_theme| iced::widget::text::Style {
                    color: Some(Color::WHITE),
                    ..Default::default()
                }),
        )
        .push(video_preview)
        .push(
            Button::new(text("Close"))
                .on_press(Message::ClosePopup)
                .padding(Padding::from([8, 16]))
                .style(|_theme, _status| button::Style {
                    background: Some(iced::Background::Color(Color::from_rgba(
                        0.4, 0.4, 0.4, 0.8,
                    ))),
                    border: iced::Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    text_color: Color::WHITE,
                    ..Default::default()
                }),
        )
        .spacing(20)
        .align_x(iced::Alignment::Center);

    let popup = Container::new(popup_content)
        .width(Length::Fixed(500.0))
        .height(Length::Fixed(450.0))
        .padding(30)
        .style(|_theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(Color::from_rgba(
                0.1, 0.1, 0.1, 0.95,
            ))),
            border: iced::Border {
                radius: 12.0.into(),
                width: 1.0,
                color: Color::from_rgba(0.5, 0.5, 0.5, 0.3),
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
                0.0, 0.0, 0.0, 0.7,
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
            Container::new(
                VideoPlayer::new(video)
                    .width(Length::Fixed(400.0))
                    .height(Length::Fixed(300.0)),
            )
            .width(Length::Fixed(400.0))
            .height(Length::Fixed(300.0))
            .style(|_theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(Color::from_rgba(
                    0.0, 0.0, 0.0, 0.9,
                ))),
                border: iced::Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: Color::from_rgba(0.5, 0.5, 0.5, 0.3),
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

            Container::new(
                text(dots)
                    .size(14)
                    .style(|_theme| iced::widget::text::Style {
                        color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.6)),
                        ..Default::default()
                    }),
            )
            .width(Length::Fixed(400.0))
            .height(Length::Fixed(300.0))
            .style(|_theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(Color::from_rgba(
                    0.2, 0.2, 0.2, 0.8,
                ))),
                border: iced::Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: Color::from_rgba(0.5, 0.5, 0.5, 0.3),
                },
                ..Default::default()
            })
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
            .into()
        }
    } else {
        Container::new(text("No video available").size(14).style(|_theme| {
            iced::widget::text::Style {
                color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.6)),
                ..Default::default()
            }
        }))
        .width(Length::Fixed(400.0))
        .height(Length::Fixed(300.0))
        .style(|_theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(Color::from_rgba(
                0.2, 0.2, 0.2, 0.8,
            ))),
            border: iced::Border {
                radius: 8.0.into(),
                width: 1.0,
                color: Color::from_rgba(0.5, 0.5, 0.5, 0.3),
            },
            ..Default::default()
        })
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .into()
    }
}

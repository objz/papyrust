use iced::{
    alignment::{Horizontal, Vertical},
    widget::{button, text, Button, Column, Container},
    Color, Element, Length, Padding,
};

use crate::{library::project::Project, Message};

pub fn build(project: &Project) -> Element<Message> {
    let title = project.meta.title.as_deref().unwrap_or("Untitled");

    let popup_content = Column::new()
        .push(
            text(title)
                .size(24)
                .style(|_theme| iced::widget::text::Style {
                    color: Some(Color::WHITE),
                    ..Default::default()
                }),
        )
        .push(
            Container::new(text("Wallpaper Preview Area").size(14).style(|_theme| {
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
            .align_y(Vertical::Center),
        )
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

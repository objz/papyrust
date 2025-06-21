use iced::{
    widget::{text, Button, Container, Row},
    Alignment, Background, Border, Element, Padding,
};

use crate::Message;

use super::state::Page;

pub fn build(_app: &crate::Papyrust) -> Element<Message> {
    let library = Button::new(text("Library"))
        .on_press(Message::SwitchPage(Page::Library))
        .padding(Padding::from([8, 16]));

    let discover = Button::new(text("Discover"))
        .on_press(Message::SwitchPage(Page::Discover))
        .padding(Padding::from([8, 16]));

    let content = Row::new()
        .push(library)
        .push(discover)
        .spacing(15)
        .align_items(Alignment::Center);

    Container::new(content)
        .padding(Padding::from([10, 20]))
        .style(|theme: &iced::Theme| iced::widget::container::Appearance {
            background: Some(Background::Color(
                theme.extended_palette().background.strong.color,
            )),
            border: Border {
                radius: 25.0.into(),
                width: 1.0,
                color: theme.extended_palette().background.weak.color,
            },
            shadow: iced::Shadow {
                color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 15.0,
            },
            ..Default::default()
        })
        .into()
}

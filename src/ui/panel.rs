use iced::{
    widget::{text, Button, Container, Row},
    Alignment, Element, Padding,
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
        .align_y(Alignment::Center);

    Container::new(content)
        .padding(Padding::from([10, 20]))
        .into()
}

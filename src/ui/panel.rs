use iced::{
    widget::{text, Button, Row},
    Element,
};

use crate::Message;

use super::state::Page;

pub fn build(_app: &crate::Papyrust) -> Element<Message> {
    let library = Button::new(text("Library")).on_press(Message::SwitchPage(Page::Library));
    let discover = Button::new(text("Discover")).on_press(Message::SwitchPage(Page::Discover));

    Row::new().push(library).push(discover).into()
}

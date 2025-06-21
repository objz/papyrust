use iced::Element;

use crate::{Message, Papyrust};

pub fn build(_app: &Papyrust) -> Element<Message> {
    iced::widget::text("Library").into()
}

use iced::{
    alignment::{Horizontal, Vertical},
    widget::{Column, Container},
    Element, Length, Padding,
};

use crate::{Message, Papyrust};

use super::{discover, library, panel, state};

pub fn build(app: &Papyrust) -> Element<Message> {
    let content = match app.current_page {
        state::Page::Discover => discover::build(app),
        state::Page::Library => library::build(app),
    };

    let main_content = Column::new()
        .push(content)
        .width(Length::Fill)
        .height(Length::Fill);

    let panel = Container::new(panel::build(app))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding::from([0, 0, 20, 0]))
        .align_x(Horizontal::Center)
        .align_y(Vertical::Bottom);

    Column::new()
        .push(main_content)
        .push(
            Container::new(panel)
                .width(Length::Fill)
                .height(Length::Fixed(80.0)),
        )
        .into()
}

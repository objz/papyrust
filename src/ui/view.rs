use iced::{widget::Column, Element};

use crate::{Message, Papyrust};

use super::{discover, library, panel, state};

pub fn build(app: &Papyrust) -> Element<Message> {
    let content = match app.current_page {
        state::Page::Discover => discover::build(app),
        state::Page::Library => library::build(app),
    };

    Column::new().push(content).push(panel::build(app)).into()
}

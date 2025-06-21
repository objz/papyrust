use crate::{Message, Papyrust};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Discover,
    Library,
}

impl Default for Page {
    fn default() -> Self {
        Page::Library
    }
}

pub fn update(_app: &mut Papyrust, message: Message) -> Papyrust {
    match message {
        Message::SwitchPage(page) => self::Papyrust { current_page: page },
    }
}

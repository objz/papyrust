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

pub fn update(app: &mut Papyrust, message: Message) {
    match message {
        Message::SwitchPage(page) => {
            app.current_page = page;
        }
        Message::Error(err) => {
            eprintln!("Error: {}", err);
        }
        _ => {}
    }
}

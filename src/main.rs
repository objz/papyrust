use iced::{Element, Task};
use ui::state;

mod library;
mod ui;

use ui::library::Library;
use ui::{state::Page, view};

pub struct Papyrust {
    pub current_page: Page,
    pub library: Library,
}

#[derive(Debug, Clone)]
pub enum Message {
    SwitchPage(Page),
    PreviewDecoded(usize, u32, u32, Vec<u8>),
    PreviewError(usize),
}

impl Papyrust {
    fn new() -> (Self, Task<Message>) {
        let mut library = Library::new();
        let first = library.next().unwrap_or_else(Task::none);
        (
            Papyrust {
                current_page: Page::default(),
                library,
            },
            first,
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        state::update(self, message)
    }

    fn view(&self) -> Element<Message> {
        view::build(self)
    }
}

fn main() -> iced::Result {
    iced::application("Papyrust", Papyrust::update, Papyrust::view)
        .theme(|_| iced::theme::Theme::GruvboxDark)
        .run_with(Papyrust::new)
}

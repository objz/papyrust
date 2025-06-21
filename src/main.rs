use iced::{Element, Task};
use ui::{
    library::Library,
    state::{self, Page},
    view,
};

mod library;
mod ui;

pub struct Papyrust {
    pub current_page: Page,
    pub library: Library,
}

#[derive(Debug, Clone)]
pub enum Message {
    SwitchPage(Page),
    Error(String),
}

impl Papyrust {
    fn new() -> (Self, Task<Message>) {
        (
            Papyrust {
                current_page: Page::default(),
                library: Library::new(),
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        state::update(self, message);
        Task::none()
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

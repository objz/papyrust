use iced::widget::image::Handle;
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
    PreviewReady(usize, Option<Handle>),
}

impl Papyrust {
    fn new() -> (Self, Task<Message>) {
        let library = Library::new();
        let tasks = library.load_previews();

        (
            Papyrust {
                current_page: Page::default(),
                library,
            },
            Task::batch(tasks),
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

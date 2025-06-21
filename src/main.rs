use iced::{Application, Command, Element, Settings};
use ui::{
    state::{self, Page},
    view,
};

mod library;
mod ui;

pub struct Papyrust {
    pub current_page: Page,
}

#[derive(Debug, Clone)]
pub enum Message {
    SwitchPage(Page),
    Error(String),
}

impl Application for Papyrust {
    type Message = Message;

    type Flags = ();

    type Theme = iced::theme::Theme;

    type Executor = iced::executor::Default;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        (
            Papyrust {
                current_page: Page::default(),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("Papyrust")
    }

    fn theme(&self) -> Self::Theme {
        iced::theme::Theme::GruvboxDark
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        state::update(self, message);
        Command::none()
    }

    fn view(&self) -> Element<Self::Message> {
        view::build(self)
    }
}

fn main() -> iced::Result {
    Papyrust::run(Settings::default())
}

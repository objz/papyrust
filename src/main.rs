use iced::widget::text;
use iced::{Application, Command, Element, Settings};

pub struct Papyrust {}

#[derive(Debug, Clone)]
pub enum Message {}

impl Application for Papyrust {
    type Message = Message;

    type Flags = ();

    type Theme = iced::theme::Theme;

    type Executor = iced::executor::Default;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        (Papyrust {}, Command::none())
    }

    fn title(&self) -> String {
        String::from("Papyrust")
    }

    fn theme(&self) -> Self::Theme {
        iced::theme::Theme::GruvboxDark
    }

    fn update(&mut self, _message: Self::Message) -> Command<Self::Message> {
        Command::none()
    }

    fn view(&self) -> Element<Self::Message> {
        text("Papyrust").into()
    }
}

fn main() -> iced::Result {
    Papyrust::run(Settings::default())
}

use iced::{Element, Font, Settings, Subscription, Task};
use library::project::Project;
use ui::state;

mod library;
mod ui;

use ui::library::Library;
use ui::{state::Page, view};

pub struct Papyrust {
    pub current_page: Page,
    pub library: Library,
    pub animation_state: usize,
}

#[derive(Debug, Clone)]
pub enum Message {
    SwitchPage(Page),
    PreviewDecoded(usize, u32, u32, Vec<u8>),
    PreviewError(usize),
    ProjectClicked(Project),
    Tick,
}

const _FIRA_BYTES: &[u8] = include_bytes!("../fonts/FiraCodeNerdFontMono-Regular.ttf");
const _UNIFONT_BYTES: &[u8] = include_bytes!("../fonts/unifont.ttf");

const _FIRA: Font = Font::with_name("FiraCode Nerd Font Mono Reg");
const _UNIFONT: Font = Font::with_name("Unifont");

impl Papyrust {
    fn new() -> (Self, Task<Message>) {
        let mut library = Library::new();
        let first = library.next().unwrap_or_else(Task::none);
        (
            Papyrust {
                current_page: Page::default(),
                library,
                animation_state: 0,
            },
            first,
        )
    }

    pub fn tick(&mut self) {
        self.animation_state = (self.animation_state + 1) % 4;
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        state::update(self, message)
    }

    fn view(&self) -> Element<Message> {
        view::build(self)
    }

    fn subscription(&self) -> Subscription<Message> {
        iced::time::every(std::time::Duration::from_millis(300)).map(|_| Message::Tick)
    }
}

fn main() -> iced::Result {
    iced::application("Papyrust", Papyrust::update, Papyrust::view)
        // .font(FIRA_BYTES)
        // .font(UNIFONT_BYTES)
        // .default_font(FIRA)
        .settings(Settings {
            default_font: Font::MONOSPACE,
            ..Default::default()
        })
        .subscription(Papyrust::subscription)
        .theme(|_| iced::theme::Theme::GruvboxDark)
        .run_with(Papyrust::new)
}

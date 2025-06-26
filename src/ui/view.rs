use iced::{
    alignment::{Horizontal, Vertical},
    widget::{image::Handle, Column, Container, Stack},
    Element, Length, Padding,
};
use iced_aw::Wrap;

use crate::{library::project::Project, Message, Papyrust};

use super::{discover, library, panel, popup, state};

pub fn build(app: &Papyrust) -> Element<Message> {
    let content = match app.current_page {
        state::Page::Discover => discover::build(app),
        state::Page::Library => library::build(app),
    };

    let main = Column::new()
        .push(content)
        .width(Length::Fill)
        .height(Length::Fill);

    let panel = Container::new(panel::build(app))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
            top: 0.0,
            right: 20.0,
            bottom: 0.0,
            left: 0.0,
        })
        .align_x(Horizontal::Center)
        .align_y(Vertical::Bottom);

    let main_content = Column::new().push(main).push(
        Container::new(panel)
            .width(Length::Fill)
            .height(Length::Fixed(80.0)),
    );

    if let Some(ref project) = app.popup_state {
        Stack::new()
            .push(main_content)
            .push(popup::build(project))
            .into()
    } else {
        main_content.into()
    }
}

pub fn create_grid<'a>(
    app: &'a Papyrust,
    projects: &'a [Project],
    preview: &'a [Option<Handle>],
) -> Element<'a, Message> {
    let mut items = Vec::new();

    for (idx, project) in projects.iter().enumerate() {
        let handle = preview.get(idx).and_then(Clone::clone);
        items.push(library::render_item(app, project, handle));
    }

    Container::new(Wrap::with_elements(items).spacing(8.0).line_spacing(8.0))
        .width(Length::Fill)
        .padding(8)
        .into()
}

use crate::{
    event::{Event, Events},
    util::StatefulList,
};

use std::{
    error::Error,
    io::{self},
};
use termion::{event::Key, input::MouseTerminal, raw::IntoRawMode, screen::AlternateScreen};
use tui::{
    backend::TermionBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Spans,
    widgets::{Block, BorderType, Borders, List, ListItem},
    Terminal,
};

pub struct App<'a> {
    items: StatefulList<&'a str>,
    file_view: bool,
    events: Events,
    should_close: bool,
}

impl<'a> App<'a> {
    pub fn new() -> Result<App<'a>, Box<dyn Error>> {
        Ok(App {
            items: StatefulList::with_items(vec![
                "▼ src",
                "    main.rs",
                "Cargo.lock",
                "Cargo.toml",
                ".gitignore",
            ]),
            file_view: false,
            events: Events::new(),
            should_close: false,
        })
    }

    pub fn close(&mut self) {
        self.should_close = true;
    }
}

pub fn render(app: &mut App) -> Result<(), Box<dyn Error>> {
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        if app.should_close {
            break;
        }
        terminal
            .draw(|f| {
                let size = f.size();

                let block = Block::default()
                    .title("LEdit")
                    .border_style(Style::default().fg(Color::Cyan))
                    .border_type(BorderType::Rounded)
                    .borders(Borders::TOP | Borders::BOTTOM);
                f.render_widget(block, size);

                let chunks: Vec<Rect>;

                if app.file_view {
                    chunks = Layout::default()
                        .margin(1)
                        .constraints(
                            [Constraint::Percentage(20), Constraint::Percentage(80)].as_ref(),
                        )
                        .direction(Direction::Horizontal)
                        .split(size);
                } else {
                    chunks = Layout::default()
                        .margin(1)
                        .constraints(
                            [Constraint::Percentage(0), Constraint::Percentage(100)].as_ref(),
                        )
                        .direction(Direction::Horizontal)
                        .split(size);
                }
                if app.file_view {
                    let files = Block::default()
                        .border_style(Style::default().fg(Color::White))
                        .borders(Borders::RIGHT)
                        .title("Files")
                        .border_type(BorderType::Plain);

                    let items: Vec<ListItem> = app
                        .items
                        .items
                        .iter()
                        .map(|i| {
                            ListItem::new(vec![Spans::from(*i)])
                                .style(Style::default().fg(Color::LightGreen).bg(Color::Black))
                        })
                        .collect();

                    // Create a List from all list items and highlight the currently selected one
                    let items = List::new(items).block(files).highlight_style(
                        Style::default()
                            .bg(Color::Gray)
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    );

                    f.render_stateful_widget(items, chunks[0], &mut app.items.state);
                }
                let editor = Block::default()
                    .border_style(Style::default().fg(Color::White))
                    .borders(Borders::RIGHT)
                    .title("Editor")
                    .border_type(BorderType::Plain);

                f.render_widget(editor, chunks[1]);
            })
            .unwrap();

        match app.events.next().unwrap() {
            Event::Input(input) => match input {
                Key::Char('q') => app.close(),
                Key::Char('f') => {
                    app.file_view = !app.file_view;
                }
                Key::Left => {
                    if app.file_view {
                        app.items.unselect();
                    }
                }
                Key::Down => {
                    if app.file_view {
                        app.items.next();
                    }
                }
                Key::Up => {
                    if app.file_view {
                        app.items.previous();
                    }
                }
                _ => {}
            },
            Event::Tick => (),
        }
    }

    Ok(())
}

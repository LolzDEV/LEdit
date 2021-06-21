use crate::{
    commands::{CommandParser, HelpCommand, QuitCommand},
    util::{
        event::{Event, Events},
        AppEvent, AppMode, StatefulList, Status, StatusLevel,
    },
};

use async_std::channel::{Receiver, Sender, TryRecvError};
use std::{
    borrow::Borrow,
    error::Error,
    io::{self},
    ops::IndexMut,
    thread, vec,
};
use termion::{event::Key, input::MouseTerminal, raw::IntoRawMode, screen::AlternateScreen};
use tui::{
    backend::TermionBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{self, Span, Spans},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Wrap},
    Terminal,
};
use uuid::Uuid;

// Main app state
pub struct App {
    items: StatefulList<Node>,
    file_view: bool,
    events: Events,
    should_close: bool,
    mode: AppMode,
    pub command_buffer: String,
    pub command_parser: CommandParser,
    pub status: Status,
    receiver: Receiver<AppEvent>,
    show_dialog: bool,
    dialog_content: String,
    dialog_title: String,
}

#[derive(Clone)]
struct Node {
    display_name: String,
    value: String,
    children: Option<Vec<Box<Node>>>,
    expanded: Option<bool>,
    uuid: uuid::Uuid,
    layer: u32,
}

// Node object, a node is an entry for the explorer that can have children
impl Node {
    fn new(
        display_name: String,
        value: String,
        children: Option<Vec<Box<Node>>>,
        expanded: Option<bool>,
        layer: u32,
    ) -> Node {
        Node {
            display_name,
            value,
            children,
            expanded,
            uuid: Uuid::new_v4(),
            layer,
        }
    }
}

// Group of nodes, it can be used to find nodes by their UUID
struct Nodes {
    nodes: Vec<Node>,
}

impl Nodes {
    fn new(nodes: Vec<Node>) -> Self {
        Nodes { nodes }
    }

    // Get node from the group by its UUID
    fn from_uuid(&mut self, uuid: Uuid) -> Option<&mut Node> {
        fn check(uuid: Uuid, node: &mut Node) -> Option<&mut Node> {
            if node.uuid == uuid {
                return Some(node);
            } else {
                if let Some(children) = &mut node.children {
                    for child in children.iter_mut() {
                        if let Some(node) = check(uuid, child) {
                            return Some(node);
                        }
                    }
                }
            }

            None
        }

        for node in self.nodes.iter_mut() {
            return check(uuid, node);
        }

        None
    }
}

// Add entry to the explorer by expanding all the nodes
fn expand(node: &Node, items: &mut Vec<ListItem>, app_list: &mut StatefulList<Node>) {
    let mut display_name = node.display_name.to_string();

    match node.expanded {
        Some(true) => {
            display_name = format!("▼ {}", display_name);
            for _ in 0..node.layer {
                display_name = format!("   {}", display_name);
            }
        }
        Some(false) => {
            display_name = format!("▶ {}", display_name);
            for _ in 0..node.layer {
                display_name = format!("   {}", display_name);
            }
        }
        None => {
            for _ in 0..node.layer {
                display_name = format!("   {}", display_name);
            }
        }
    }

    app_list.items.push(Node {
        display_name: display_name.to_string(),
        value: display_name.to_string(),
        children: None,
        expanded: None,
        uuid: node.uuid,
        layer: 0,
    });

    items.push(
        ListItem::new(vec![Spans::from(display_name.to_string())])
            .style(Style::default().fg(Color::LightGreen).bg(Color::Black)),
    );

    if let Some(true) = node.expanded {
        if let Some(children) = node.children.clone() {
            for child in children.iter() {
                expand(child, items, app_list);
            }
        }
    }
}

impl App {
    pub fn new(tx: Sender<AppEvent>, rx: Receiver<AppEvent>) -> Result<App, Box<dyn Error>> {
        Ok(App {
            items: StatefulList::new(),
            file_view: true,
            events: Events::new(),
            should_close: false,
            mode: AppMode::NormalMode,
            command_buffer: "".to_string(),
            command_parser: CommandParser::new(tx.clone()),
            status: Status::default(),
            receiver: rx,
            show_dialog: false,
            dialog_content: String::new(),
            dialog_title: String::new(),
        })
    }

    pub fn setup_commands(&mut self) {
        self.command_parser.add_command(Box::new(QuitCommand));
        self.command_parser
            .add_command(Box::new(HelpCommand::new(&self.command_parser.commands)));
    }

    pub fn close(&mut self) {
        self.should_close = true;
    }
}

// Render method, this is the main loop that renders all the TUI
pub fn render(app: &mut App) -> Result<(), Box<dyn Error>> {
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut file_list = Nodes::new(vec![
        Node::new(
            String::from("src"),
            String::from("src"),
            Some(vec![Box::new(Node::new(
                String::from("main.rs"),
                String::from("main.rs"),
                Some(vec![Box::new(Node::new(
                    String::from("main.rs"),
                    String::from("main.rs"),
                    None,
                    None,
                    2,
                ))]),
                Some(false),
                1,
            ))]),
            Some(false),
            0,
        ),
        Node::new(
            String::from("Cargo.toml"),
            String::from("Cargo.toml"),
            None,
            None,
            0,
        ),
        Node::new(
            String::from("Cargo.lock"),
            String::from("Cargo.lock"),
            None,
            None,
            0,
        ),
        Node::new(
            String::from(".gitignore"),
            String::from(".gitignore"),
            None,
            None,
            0,
        ),
    ]);

    loop {
        if app.should_close {
            break;
        }
        terminal
            .draw(|f| {
                // Size for the current frame
                let size = f.size();

                if app.show_dialog {
                    let dialog_block = Block::default()
                        .title(app.dialog_title.clone())
                        .border_style(Style::default().fg(Color::Red))
                        .border_type(BorderType::Rounded)
                        .borders(Borders::ALL);

                    let continue_block = Block::default().borders(Borders::NONE);

                    let dialog_paragraph = Paragraph::new(app.dialog_content.clone())
                        .block(dialog_block)
                        .alignment(Alignment::Center);

                    let dialog_chunks = Layout::default()
                        .constraints([Constraint::Percentage(90), Constraint::Percentage(10)])
                        .direction(Direction::Vertical)
                        .split(Rect {
                            x: (size.x + (size.width / 2)) - (size.width / 2) / 2,
                            y: (size.y + (size.height / 2)) - (size.height / 2) / 2,
                            height: size.height / 2,
                            width: size.width / 2,
                        });

                    f.render_widget(
                        dialog_paragraph,
                        Rect {
                            x: (size.x + (size.width / 2)) - (size.width / 2) / 2,
                            y: (size.y + (size.height / 2)) - (size.height / 2) / 2,
                            height: size.height / 2,
                            width: size.width / 2,
                        },
                    );
                    f.render_widget(
                        Paragraph::new("Press <ENTER> to close")
                            .block(continue_block)
                            .alignment(Alignment::Center),
                        dialog_chunks[1],
                    );
                }
                // Main block
                let block = Block::default()
                    .title("LEdit")
                    .border_style(Style::default().fg(Color::Cyan))
                    .border_type(BorderType::Rounded)
                    .borders(Borders::TOP | Borders::BOTTOM);
                f.render_widget(block, size);

                let top_chunks: Vec<Rect>;
                let chunks: Vec<Rect>;
                let bottom_chunks: Vec<Rect>;

                top_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(4), Constraint::Percentage(96)])
                    .margin(1)
                    .split(size);

                // If the command view is open set its with to the 20% of the frame and the rest to the 80%
                if let AppMode::CommandMode = app.mode {
                    bottom_chunks = Layout::default()
                        .margin(0)
                        .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
                        .direction(Direction::Vertical)
                        .split(top_chunks[1]);
                } else {
                    bottom_chunks = Layout::default()
                        .margin(0)
                        .constraints([Constraint::Percentage(100), Constraint::Percentage(0)])
                        .direction(Direction::Vertical)
                        .split(top_chunks[1]);
                }

                // If the explorer is open set its width to the 20% of the frame and the editor's width to the 80%, else the editor should have a width of 100%
                if app.file_view {
                    chunks = Layout::default()
                        .margin(1)
                        .constraints(
                            [Constraint::Percentage(20), Constraint::Percentage(80)].as_ref(),
                        )
                        .direction(Direction::Horizontal)
                        .split(bottom_chunks[0]);
                } else {
                    chunks = Layout::default()
                        .margin(1)
                        .constraints(
                            [Constraint::Percentage(0), Constraint::Percentage(100)].as_ref(),
                        )
                        .direction(Direction::Horizontal)
                        .split(bottom_chunks[0]);
                }

                // If the explorer is open, render it
                if app.file_view {
                    let files = Block::default()
                        .border_style(Style::default().fg(if let AppMode::NormalMode = app.mode {
                            Color::LightBlue
                        } else {
                            Color::White
                        }))
                        .borders(Borders::ALL)
                        .title("Explorer")
                        .border_type(BorderType::Plain);

                    let mut items: Vec<ListItem> = Vec::new();
                    app.items.items = Vec::new();
                    for item in file_list.nodes.iter() {
                        expand(item, &mut items, &mut app.items);
                    }

                    // Create a List from all list items and highlight the currently selected one
                    let items = List::new(items).block(files).highlight_style(
                        Style::default()
                            .bg(Color::Gray)
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    );

                    f.render_stateful_widget(items, chunks[0], &mut app.items.state);
                }

                let status_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                    .split(top_chunks[0]);

                // Status bar block
                let mode_bar = Block::default()
                    .border_style(Style::default().bg(Color::Blue).fg(Color::White))
                    .borders(Borders::empty())
                    .style(Style::default().bg(Color::Rgb(0, 0, 255)));

                let status_bar = Block::default()
                    .border_style(Style::default().bg(Color::Rgb(0, 0, 255)).fg(Color::White))
                    .borders(Borders::empty())
                    .style(Style::default().bg(Color::Rgb(0, 0, 255)));
                // Current mode as string
                let current_mode = match app.mode {
                    AppMode::InsertMode => "Insert Mode",
                    AppMode::CommandMode => "Command Mode",
                    AppMode::NormalMode => "Normal Mode",
                };

                // Status paragraph
                let mode_paragraph =
                    Paragraph::new(Spans::from(format!("Current Mode: {}", current_mode)))
                        .wrap(Wrap { trim: true })
                        .block(mode_bar)
                        .style(Style::default().add_modifier(Modifier::BOLD));

                let status_paragraph = Paragraph::new(Spans::from(Span::styled(
                    app.status.text.clone(),
                    Style::default()
                        .fg(match app.status.level {
                            StatusLevel::ERROR => Color::Red,
                            StatusLevel::INFO => Color::LightGreen,
                            StatusLevel::WARNING => Color::Yellow,
                        })
                        .add_modifier(Modifier::BOLD),
                )))
                .wrap(Wrap { trim: true })
                .block(status_bar)
                .style(Style::default().add_modifier(Modifier::BOLD));

                f.render_widget(mode_paragraph, status_chunks[0]);
                f.render_widget(status_paragraph, status_chunks[1]);

                // If the command view is open, render it
                if let AppMode::CommandMode = app.mode {
                    let command_view = Block::default()
                        .title("Commands")
                        .border_style(Style::default().fg(Color::LightBlue))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Plain);

                    let command_paragraph =
                        Paragraph::new(format!("> {}", app.command_buffer)).block(command_view);

                    f.render_widget(command_paragraph, bottom_chunks[1]);
                    f.set_cursor(
                        bottom_chunks[1].x + app.command_buffer.len() as u16 + 3,
                        bottom_chunks[1].y + 1,
                    );
                }

                // Editor block
                let editor = Block::default()
                    .border_style(Style::default().fg(if let AppMode::InsertMode = app.mode {
                        Color::LightBlue
                    } else {
                        Color::White
                    }))
                    .borders(Borders::ALL)
                    .title("Editor")
                    .border_type(BorderType::Plain);

                f.render_widget(editor, chunks[1]);
            })
            .unwrap();

        // Check for events
        match app.events.next().unwrap() {
            Event::Input(input) => match app.mode {
                AppMode::NormalMode => match input {
                    Key::Char('\n') => {
                        if app.show_dialog {
                            app.show_dialog = false;
                        }
                    }
                    // If 'q' is pressed, quit the app
                    Key::Char('q') => app.close(),
                    // If 'f' is pressed open/close the explorer
                    Key::Char('f') => app.file_view = !app.file_view,
                    // If 'c' is pressed go in command mode
                    Key::Char('c') => app.mode = AppMode::CommandMode,
                    // If 'i' is pressed go in insert mode
                    Key::Char('i') => app.mode = AppMode::InsertMode,
                    // If the left arrow is pressed unselect the entry from the explorer
                    Key::Left => {
                        if app.file_view {
                            app.items.unselect();
                        }
                    }
                    // If the down arrow is pressed select the next entry in the explorer
                    Key::Down => {
                        if app.file_view {
                            app.items.next();
                        }
                    }
                    // If the up arrow is pressed select the previous entry in the explorer
                    Key::Up => {
                        if app.file_view {
                            app.items.previous();
                        }
                    }
                    // If the right arrow is pressed expand the selected node
                    Key::Right => {
                        if let Some(ind) = app.items.state.selected() {
                            if let Some(node) =
                                file_list.from_uuid(app.items.items.index_mut(ind).uuid)
                            {
                                if let Some(exp) = node.expanded {
                                    node.expanded = Some(!exp);
                                }
                            }
                        }
                    }
                    _ => {}
                },
                AppMode::InsertMode => match input {
                    Key::Esc => app.mode = AppMode::NormalMode,
                    _ => {}
                },
                AppMode::CommandMode => match input {
                    Key::Esc => app.mode = AppMode::NormalMode,
                    Key::Char('\n') => {
                        if app.command_buffer != "" {
                            match app.command_parser.parse(app.command_buffer.clone()) {
                                Ok((cmd, tx)) => {
                                    let mut args: Vec<String> = app
                                        .command_buffer
                                        .clone()
                                        .split(' ')
                                        .map(|a| String::from(a))
                                        .collect();
                                    args.remove(0);
                                    if let Err(crate::commands::CommandError::InvalidSyntax) =
                                        cmd.execute(tx, &args)
                                    {
                                        app.status = Status {
                                            text: format!(
                                                "Invalid syntax! Type `help {}`",
                                                cmd.get_name()
                                            )
                                            .to_string(),
                                            level: crate::util::StatusLevel::ERROR,
                                        }
                                    }
                                    app.command_buffer = String::new();
                                }
                                Err(e) => match e {
                                    crate::commands::CommandError::NotFound => {
                                        app.status = Status {
                                            text: "Command not found!".to_string(),
                                            level: crate::util::StatusLevel::ERROR,
                                        }
                                    }
                                    crate::commands::CommandError::InvalidSyntax => todo!(),
                                },
                            }
                        }
                    }
                    Key::Char(c) => app.command_buffer.push(c),
                    Key::Backspace => {
                        app.command_buffer.pop();
                    }
                    _ => {}
                },
            },
            Event::Tick => (),
        }

        match app.receiver.try_recv() {
            Ok(AppEvent::Close) => app.close(),
            Ok(AppEvent::ShowDialog((title, content))) => {
                app.show_dialog = true;
                app.dialog_content = content;
                app.mode = AppMode::NormalMode;
                app.dialog_title = title;
            }
            Err(e) => {
                if e == TryRecvError::Closed {
                    app.status = Status {
                        text: format!("Error receiving application events: {:?}", &e),
                        level: crate::util::StatusLevel::ERROR,
                    }
                }
            }
        }
    }

    Ok(())
}

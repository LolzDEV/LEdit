use std::cmp::Ordering;

use crate::{
    commands::{CommandParser, HelpCommand, OpenCommand, QuitCommand},
    util::{
        event::{Event, Events},
        AppEvent, AppMode, NodeType, StatefulList, Status, StatusLevel,
    },
};

use async_std::channel::{Receiver, Sender, TryRecvError};
use std::{
    error::Error,
    io::{self},
    ops::IndexMut,
    path::{Path, PathBuf},
    vec,
};
use termion::{event::Key, input::MouseTerminal, raw::IntoRawMode, screen::AlternateScreen};
use tui::{
    backend::TermionBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
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
    working_path: Option<String>,
    file_list: Nodes,
}

#[derive(Clone, Debug)]
struct Node {
    display_name: String,
    value: String,
    children: Option<Vec<Box<Node>>>,
    expanded: Option<bool>,
    uuid: uuid::Uuid,
    layer: u32,
    node_type: NodeType,
}

// Node object, a node is an entry for the explorer that can have children
impl Node {
    fn new(
        display_name: String,
        value: String,
        children: Option<Vec<Box<Node>>>,
        expanded: Option<bool>,
        layer: u32,
        node_type: NodeType,
    ) -> Node {
        Node {
            display_name,
            value,
            children,
            expanded,
            uuid: Uuid::new_v4(),
            layer,
            node_type,
        }
    }

    fn cmp(&self, other: &Self) -> Ordering {
        if let NodeType::Info = self.node_type {
            if let NodeType::Info = other.node_type {
                return self.display_name.cmp(&other.display_name);
            } else {
                return Ordering::Greater;
            }
        }

        if let NodeType::Directory = self.node_type {
            if let NodeType::Directory = other.node_type {
                return self.display_name.cmp(&other.display_name);
            } else if let NodeType::Info = other.node_type {
                return Ordering::Less;
            } else if let NodeType::File = other.node_type {
                return Ordering::Greater;
            }
        }

        if let NodeType::File = self.node_type {
            if let NodeType::Directory = other.node_type {
                return Ordering::Less;
            } else if let NodeType::Info = other.node_type {
                return Ordering::Less;
            } else if let NodeType::File = other.node_type {
                return self.display_name.cmp(&other.display_name);
            }
        }

        Ordering::Equal
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
    fn from_uuid(&mut self, uuid: &Uuid) -> Option<&mut Node> {
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
            if let Some(nd) = check(uuid.clone(), node) {
                return Some(nd);
            }
        }

        None
    }
}

// Add entry to the explorer by expanding all the nodes
fn expand(node: Node, items: &mut Vec<ListItem>, app_list: &mut StatefulList<Node>) {
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
        uuid: node.uuid.clone(),
        layer: 0,
        node_type: NodeType::File,
    });

    items.push(
        ListItem::new(vec![Spans::from(display_name.to_string())]).style(
            Style::default()
                .fg(if let NodeType::Directory = node.node_type {
                    if node.display_name.starts_with('.') {
                        Color::Gray
                    } else {
                        Color::LightBlue
                    }
                } else if let NodeType::Info = node.node_type {
                    Color::Gray
                } else {
                    if node.display_name.starts_with('.') {
                        Color::Gray
                    } else {
                        Color::LightGreen
                    }
                })
                .bg(Color::Black),
        ),
    );

    if let Some(true) = node.expanded {
        if let Some(children) = node.children.clone() {
            for child in children.iter() {
                expand(*child.clone(), items, app_list);
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
            working_path: None,
            file_list: Nodes::new(Vec::new()),
        })
    }

    pub fn setup_commands(&mut self) {
        self.command_parser.add_command(Box::new(QuitCommand));
        self.command_parser.add_command(Box::new(OpenCommand));
        self.command_parser
            .add_command(Box::new(HelpCommand::new(&self.command_parser.commands)));
    }

    pub fn close(&mut self) {
        self.should_close = true;
    }

    pub fn load_explorer(&mut self) -> Result<(), Box<dyn Error>> {
        fn expand_path(dir: PathBuf, level: u32) -> Result<Node, Box<dyn Error>> {
            let mut node: Node = Node::new(
                dir.file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string()
                    .clone(),
                dir.to_str().unwrap().to_string().clone(),
                None,
                None,
                level,
                NodeType::Directory,
            );
            if dir.exists() {
                let mut children = Vec::new();
                if dir.is_dir() {
                    for entry in dir.read_dir()? {
                        if let Ok(en) = entry {
                            if let Ok(child) = expand_path(en.path(), level + 1) {
                                children.push(Box::new(child));
                            }
                        }
                    }
                    node.children = Some(children);
                    node.expanded = Some(false);
                    node.node_type = NodeType::Directory;
                } else {
                    node.node_type = NodeType::File;
                }
            }

            Ok(node)
        }

        if let Some(workspace_path) = &self.working_path {
            let mut expl = Vec::new();
            let path = Path::new(workspace_path);
            if path.exists() {
                if path.is_dir() {
                    for entry in path.read_dir()? {
                        if let Ok(en) = entry {
                            if let Ok(nd) = expand_path(en.path(), 0) {
                                expl.push(nd.clone());
                            }
                        }
                    }
                }
            }
            self.file_list.nodes = expl;
        } else {
            self.file_list.nodes = vec![Node::new(
                "Empty workspace".to_string(),
                "".to_string(),
                None,
                None,
                0,
                NodeType::Info,
            )];
        }

        self.file_list.nodes.sort_by(|a, b| b.cmp(a));

        Ok(())
    }
}

// Render method, this is the main loop that renders all the TUI
pub fn render(app: &mut App) -> Result<(), Box<dyn Error>> {
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    if let Err(_) = app.load_explorer() {
        app.status = Status {
            text: "Cannot load explorer!".to_string(),
            level: StatusLevel::ERROR,
        }
    }

    loop {
        // If the app should close, close it
        if app.should_close {
            break;
        }
        terminal
            .draw(|f| {
                // Size for the current frame
                let size = f.size();

                // If a dialog is open, render it
                if app.show_dialog {
                    // Block of the dialog
                    let dialog_block = Block::default()
                        .title(app.dialog_title.clone())
                        .border_style(Style::default().fg(Color::Red))
                        .border_type(BorderType::Rounded)
                        .borders(Borders::ALL);

                    // Block of the "continue" text
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
                    for item in app.file_list.nodes.iter() {
                        expand(item.clone(), &mut items, &mut app.items);
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
                    // If `enter` is pressed and the dialog is open, close it
                    Key::Char('\n') => {
                        if app.show_dialog {
                            app.show_dialog = false;
                        }
                    }
                    // If 'q' is pressed, quit the app
                    Key::Char('q') => {
                        if !app.show_dialog {
                            app.close()
                        }
                    }
                    // If 'f' is pressed open/close the explorer
                    Key::Char('f') => {
                        if !app.show_dialog {
                            app.file_view = !app.file_view
                        }
                    }
                    // If 'c' is pressed go in command mode
                    Key::Char('c') => {
                        if !app.show_dialog {
                            app.mode = AppMode::CommandMode
                        }
                    }
                    // If 'i' is pressed go in insert mode
                    Key::Char('i') => {
                        if !app.show_dialog {
                            app.mode = AppMode::InsertMode
                        }
                    }
                    // If the left arrow is pressed unselect the entry from the explorer
                    Key::Esc => {
                        if !app.show_dialog {
                            if app.file_view {
                                app.items.unselect();
                            }
                        }
                    }
                    // If the down arrow is pressed select the next entry in the explorer
                    Key::Down => {
                        if !app.show_dialog {
                            if app.file_view {
                                app.items.next();
                            }
                        }
                    }
                    // If the up arrow is pressed select the previous entry in the explorer
                    Key::Up => {
                        if !app.show_dialog {
                            if app.file_view {
                                app.items.previous();
                            }
                        }
                    }
                    // If the right arrow is pressed expand the selected node
                    Key::Char(' ') => {
                        if !app.show_dialog {
                            if let Some(ind) = app.items.state.selected() {
                                if let Some(node) = app
                                    .file_list
                                    .from_uuid(&app.items.items.index_mut(ind).uuid)
                                {
                                    if let Some(exp) = node.expanded {
                                        node.expanded = Some(!exp);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                },
                // When the app is in insert mode
                AppMode::InsertMode => match input {
                    // If `esc` is pressed go in normal mode
                    Key::Esc => app.mode = AppMode::NormalMode,
                    _ => {}
                },
                // When the app is in command mode
                AppMode::CommandMode => match input {
                    // If `esc` is pressed go in normal mode
                    Key::Esc => app.mode = AppMode::NormalMode,
                    // If `enter` is pressed and the command buffer is not empty
                    Key::Char('\n') => {
                        if app.command_buffer != "" {
                            // Parse the command with te command parser
                            match app.command_parser.parse(app.command_buffer.clone()) {
                                Ok((cmd, tx)) => {
                                    // Get the arguments
                                    let mut args: Vec<String> = app
                                        .command_buffer
                                        .clone()
                                        .split(' ')
                                        .map(|a| String::from(a))
                                        .collect();
                                    args.remove(0);
                                    if let Err(crate::commands::CommandError::InvalidSyntax) =
                                        cmd.execute(tx, &args)
                                    // Execute the command and check for errors
                                    {
                                        // If there is an error show it in the status
                                        app.status = Status {
                                            text: format!(
                                                "Invalid syntax! Type `help {}`",
                                                cmd.get_name()
                                            )
                                            .to_string(),
                                            level: crate::util::StatusLevel::ERROR,
                                        }
                                    }
                                }
                                Err(e) => match e {
                                    // If the command is not found, show it in the status
                                    crate::commands::CommandError::NotFound => {
                                        app.status = Status {
                                            text: "Command not found!".to_string(),
                                            level: crate::util::StatusLevel::ERROR,
                                        }
                                    }
                                    // If the command has an invalid syntaxt, show it in the status
                                    crate::commands::CommandError::InvalidSyntax => {
                                        app.status = Status {
                                            text: "Invalid syntax!".to_string(),
                                            level: crate::util::StatusLevel::ERROR,
                                        }
                                    }
                                    // If an execution error is throwed
                                    crate::commands::CommandError::ExecutionError(e) => {
                                        // If a description is provided, show it in the status
                                        if let Some(e) = e {
                                            app.status = Status {
                                                text: format!(
                                                    "Error while executing the command: {}",
                                                    &e
                                                ),
                                                level: crate::util::StatusLevel::ERROR,
                                            }
                                        // Else say that an unknown error has been catched
                                        } else {
                                            app.status = Status {
                                                text: "Error while executing the command: Unknown error"
                                                    .to_string(),
                                                level: crate::util::StatusLevel::ERROR,
                                            }
                                        }
                                    }
                                },
                            }
                            // Free the command buffer
                            app.command_buffer = String::new();
                        }
                    }
                    // If a char key is pressed, add that character to the command buffer
                    Key::Char(c) => app.command_buffer.push(c),
                    // If backspace is pressed remove tha last character from the command buffer
                    Key::Backspace => {
                        app.command_buffer.pop();
                    }
                    _ => {}
                },
            },
            Event::Tick => (),
        }

        // This checks the receiver that is bound to a sender used by commands
        match app.receiver.try_recv() {
            // Close the application if requested
            Ok(AppEvent::Close) => app.close(),
            // Show a dialog with the given information
            Ok(AppEvent::ShowDialog((title, content))) => {
                app.show_dialog = true;
                app.dialog_content = content;
                app.mode = AppMode::NormalMode;
                app.dialog_title = title;
            }
            // Set the status with the given information
            Ok(AppEvent::SetStatus(s)) => {
                app.status = s;
            }
            // Set the workspace to the given path
            Ok(AppEvent::SetWorkspace(w)) => {
                app.working_path = Some(w);
                if let Err(_) = app.load_explorer() {
                    app.status = Status {
                        text: "Error while loading the explorer".to_string(),
                        level: StatusLevel::ERROR,
                    };
                }
            }
            // If there is an error while receiving, show it in the status
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

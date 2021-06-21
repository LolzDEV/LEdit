use crate::{
    event::{Event, Events},
    util::StatefulList,
};

use std::{
    error::Error,
    io::{self},
    ops::IndexMut,
    vec,
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
use uuid::Uuid;

// Main app state
pub struct App {
    items: StatefulList<Node>,
    file_view: bool,
    events: Events,
    should_close: bool,
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
    pub fn new() -> Result<App, Box<dyn Error>> {
        Ok(App {
            items: StatefulList::new(),
            file_view: false,
            events: Events::new(),
            should_close: false,
        })
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

                // Main block
                let block = Block::default()
                    .title("LEdit")
                    .border_style(Style::default().fg(Color::Cyan))
                    .border_type(BorderType::Rounded)
                    .borders(Borders::TOP | Borders::BOTTOM);
                f.render_widget(block, size);

                let chunks: Vec<Rect>;

                // If the explorer is open set its width to the 20% of the frame and the editor's width to the 80%, else the editor should have a width of 100%
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

                // If the explorer is open, render it
                if app.file_view {
                    let files = Block::default()
                        .border_style(Style::default().fg(Color::White))
                        .borders(Borders::RIGHT)
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

                // Editor block
                let editor = Block::default()
                    .border_style(Style::default().fg(Color::White))
                    .borders(Borders::RIGHT)
                    .title("Editor")
                    .border_type(BorderType::Plain);

                f.render_widget(editor, chunks[1]);
            })
            .unwrap();

        // Check for events
        match app.events.next().unwrap() {
            Event::Input(input) => match input {
                // If 'q' is pressed, quit the app
                Key::Char('q') => app.close(),
                // If 'f' is pressed open/close the explorer
                Key::Char('f') => {
                    app.file_view = !app.file_view;
                }
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
                        if let Some(node) = file_list.from_uuid(app.items.items.index_mut(ind).uuid)
                        {
                            if let Some(exp) = node.expanded {
                                node.expanded = Some(!exp);
                            }
                        }
                    }
                }
                _ => {}
            },
            Event::Tick => (),
        }
    }

    Ok(())
}

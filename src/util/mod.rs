use tui::widgets::ListState;
pub mod event;

pub struct StatefulList<T> {
    pub state: ListState,
    pub items: Vec<T>,
}

impl<T> StatefulList<T> {
    pub fn new() -> StatefulList<T> {
        StatefulList {
            state: ListState::default(),
            items: Vec::new(),
        }
    }

    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn unselect(&mut self) {
        self.state.select(None);
    }
}

pub enum AppMode {
    InsertMode,
    CommandMode,
    NormalMode,
}

#[derive(Clone, Copy)]
pub enum StatusLevel {
    INFO,
    WARNING,
    ERROR,
}

#[derive(Clone)]
pub struct Status {
    pub text: String,
    pub level: StatusLevel,
}

impl Default for Status {
    fn default() -> Self {
        Status {
            text: String::new(),
            level: StatusLevel::INFO,
        }
    }
}

pub enum AppEvent {
    Close,
    ShowDialog((String, String)),
    SetStatus(Status),
    SetWorkspace(String),
}

#[derive(Clone, Copy, Debug)]
pub enum NodeType {
    File,
    Directory,
    Info,
}

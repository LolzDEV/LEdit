use tui::{style::Color, widgets::ListState};
pub mod event;
use css_color_parser::Color as CssColor;
use serde_derive::{Deserialize, Serialize};

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

#[allow(dead_code)]
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
    File = 1,
    Directory = 2,
    Info = 0,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Config {
    pub logs_directory: Option<String>,
    pub theme: Option<Theme>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            logs_directory: Some(String::from("~/.ledit/logs")),
            theme: Some(Theme::default()),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Theme {
    pub status_bar_background: Option<String>,
    pub status_bar_foreground: Option<String>,
    pub explorer_background: Option<String>,
    pub explorer_selected_background: Option<String>,
    pub explorer_selected_foreground: Option<String>,
    pub explorer_directory_foreground: Option<String>,
    pub explorer_file_foreground: Option<String>,
    pub explorer_info_foreground: Option<String>,
    pub active_view_border: Option<String>,
    pub view_border: Option<String>,
    pub editor_background: Option<String>,
    pub commands_view_background: Option<String>,
    pub commands_view_foreground: Option<String>,
    pub explorer_hidden_foreground: Option<String>,
    pub app_background: Option<String>,
    pub app_foreground: Option<String>,
    pub status_error: Option<String>,
    pub status_warning: Option<String>,
    pub status_info: Option<String>,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            app_background: Some("#000000".to_string()),
            app_foreground: Some("#0000FF".to_string()),
            status_bar_background: Some("#0000ff".to_string()),
            status_bar_foreground: Some("#FFFFFF".to_string()),
            explorer_background: Some("#000000".to_string()),
            explorer_selected_background: Some("#808080".to_string()),
            explorer_selected_foreground: Some("#000000".to_string()),
            explorer_directory_foreground: Some("#00FF00".to_string()),
            explorer_file_foreground: Some("#0000FF".to_string()),
            explorer_info_foreground: Some("#808080".to_string()),
            explorer_hidden_foreground: Some("#808080".to_string()),
            active_view_border: Some("#0084FF".to_string()),
            view_border: Some("#FFFFFF".to_string()),
            editor_background: Some("#000000".to_string()),
            commands_view_background: Some("#000000".to_string()),
            commands_view_foreground: Some("#FFFFFF".to_string()),
            status_info: Some("#00FF00".to_string()),
            status_warning: Some("FF9100".to_string()),
            status_error: Some("#FF0000".to_string()),
        }
    }
}

impl Theme {
    pub fn get_color_for(value: Option<String>) -> Option<Color> {
        if let Some(c) = value {
            return Some(Color::Rgb(
                c.parse::<CssColor>()
                    .unwrap_or(CssColor {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 1.0,
                    })
                    .r,
                c.parse::<CssColor>()
                    .unwrap_or(CssColor {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 1.0,
                    })
                    .g,
                c.parse::<CssColor>()
                    .unwrap_or(CssColor {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 1.0,
                    })
                    .b,
            ));
        } else {
            return None;
        }
    }
}

use std::{
    fs::File,
    io::{LineWriter, Write},
    path::PathBuf,
};

use chrono::Local;

pub struct Logger {
    logs_path: PathBuf,
    logs: Vec<String>,
}

impl Logger {
    pub fn new(logs_path: String) -> Self {
        Logger {
            logs_path: if let Ok(path) = shellexpand::full(&logs_path) {
                PathBuf::from(&*path)
            } else {
                PathBuf::from(logs_path)
            },
            logs: Vec::new(),
        }
    }

    pub fn log(&mut self, level: LogLevel, message: String) {
        let current_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let level_str = match level {
            LogLevel::ERROR => String::from("ERROR"),
            LogLevel::WARN => String::from("WARN"),
            LogLevel::INFO => String::from("INFO"),
        };

        self.logs
            .push(format!("[{}][{}]: {}", level_str, current_time, message));
    }

    pub fn write(&mut self) {
        if let Ok(file) = File::create(&self.logs_path.join("latest.log")) {
            let mut writer = LineWriter::new(file);
            for log in self.logs.iter() {
                if let Err(e) = writer.write(format!("{}\n", &log).as_bytes()) {
                    panic!("Error while trying to write the logs!: {}", e.to_string());
                }
            }
        }
    }
}

pub enum LogLevel {
    INFO,
    WARN,
    ERROR,
}

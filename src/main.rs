mod application;
use application::render;
use util::Config;
use util::Status;
mod commands;
mod logs;
mod util;

use crate::application::App;
use async_std::channel::unbounded;
use std::env;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

// Program entry point
fn main() -> Result<(), Box<dyn Error>> {
    let mut config = Config::default();

    // If the app directory doesn't exist, create it
    if let Ok(path) = shellexpand::full("~/.ledit") {
        let dir = PathBuf::from(Path::new(&*path));
        if !dir.exists() {
            if let Err(_) = fs::create_dir(&dir) {
                eprintln!("Error while creating the application directory!")
            }
            if let Err(_) = fs::create_dir(dir.join("logs")) {
                eprintln!("Error while creating the application logs directory!")
            }
            if let Ok(mut config_file) = File::create(dir.join("config.toml")) {
                if let Err(_) =
                    config_file.write_all(toml::to_string(&Config::default()).unwrap().as_bytes())
                {
                    eprintln!("Error while creating the default configuration file!")
                }
            }
        }
    }

    // If there is a configuration file, load the current configuration from it
    if let Ok(path) = shellexpand::full("~/.ledit/config.toml") {
        let dir = PathBuf::from(Path::new(&*path));
        if dir.exists() {
            let mut buf = String::new();
            if let Ok(mut file) = File::open(dir) {
                if let Ok(_) = file.read_to_string(&mut buf) {
                    config = toml::from_str(&buf)
                        .expect("Cannot load the config file, check the syntax!");
                }
            }
        }
    }

    let args: Vec<String> = env::args().collect();

    let (tx, rx) = unbounded();

    // Application instance
    let mut app = App::new(tx.clone(), rx, config)?;

    // Register the commands
    app.setup_commands();

    // If there is at least an argument use it as workspace folder
    if args.len() > 1 {
        app.working_path = Some(args[1].clone());
        if let Err(_) = app.load_explorer() {
            app.status = Status {
                text: format!("Failed to open the workspace from {}", args[1].clone()),
                level: util::StatusLevel::ERROR,
            }
        }
    }

    // Run the render loop for the given app instance
    render(&mut app)?;

    // Exit the program with no errors
    Ok(())
}

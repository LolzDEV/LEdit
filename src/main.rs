mod application;
use application::render;
use util::Status;
mod commands;
mod util;

use crate::application::App;
use async_std::channel::unbounded;
use std::env;
use std::error::Error;

// Program entry point
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();

    let (tx, rx) = unbounded();

    // Application instance
    let mut app = App::new(tx.clone(), rx)?;

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

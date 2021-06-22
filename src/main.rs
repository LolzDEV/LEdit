mod application;
use application::render;
mod commands;
mod util;

use crate::application::App;
use async_std::channel::unbounded;
use std::error::Error;

// Program entry point
fn main() -> Result<(), Box<dyn Error>> {
    let (tx, rx) = unbounded();

    // Application instance
    let mut app = App::new(tx.clone(), rx)?;

    app.setup_commands();
    // Run the render loop for the given app instance
    render(&mut app)?;

    // Exit the program with no errors
    Ok(())
}

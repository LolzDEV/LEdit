mod application;
mod event;
mod util;

use application::render;

use crate::application::App;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let mut app = App::new()?;

    render(&mut app)?;

    Ok(())
}

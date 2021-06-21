use std::{borrow::Borrow, collections::HashMap};

use async_std::channel::Sender;
use futures::executor::block_on;

use crate::{application::App, util::AppEvent};

pub trait Command {
    fn get_name(&self) -> String;
    fn get_aliases(&self) -> Vec<String>;
    fn execute(&self, tx: Sender<AppEvent>, args: &Vec<String>) -> Result<(), CommandError>;
    fn get_description(&self) -> String;
}

pub enum CommandError {
    NotFound,
    InvalidSyntax,
}

pub struct CommandParser {
    pub commands: Vec<Box<dyn Command>>,
    transmitter: Sender<AppEvent>,
}

impl CommandParser {
    pub fn new(transmitter: Sender<AppEvent>) -> Self {
        CommandParser {
            commands: Vec::new(),
            transmitter,
        }
    }

    pub fn add_command(&mut self, command: Box<dyn Command>) {
        self.commands.push(command);
    }

    pub fn parse(
        &mut self,
        buffer: String,
    ) -> Result<(&Box<dyn Command>, Sender<AppEvent>), CommandError> {
        let mut splitted: Vec<String> = buffer.split(' ').map(|p| String::from(p)).collect();
        for cmd in self.commands.iter() {
            if splitted[0] == cmd.get_name() {
                &splitted.remove(0);
                return Ok((&Box::new(cmd), self.transmitter.clone()));
            } else {
                for alias in cmd.get_aliases().iter() {
                    if splitted[0] == *alias {
                        &splitted.remove(0);
                        return Ok((&Box::new(cmd), self.transmitter.clone()));
                    }
                }
            }
        }

        Err(CommandError::NotFound)
    }
}

pub struct QuitCommand;

impl Command for QuitCommand {
    fn get_name(&self) -> String {
        String::from("quit")
    }

    fn get_aliases(&self) -> Vec<String> {
        vec![String::from("q")]
    }

    fn execute(&self, tx: Sender<AppEvent>, args: &Vec<String>) -> Result<(), CommandError> {
        block_on(tx.send(AppEvent::Close));

        Ok(())
    }

    fn get_description(&self) -> String {
        "Quits the application without saving.\nUsage: quit".to_string()
    }
}

pub struct HelpCommand {
    pub commands: HashMap<String, String>,
}

impl HelpCommand {
    pub fn new(commands: &Vec<Box<dyn Command>>) -> Self {
        let mut cmds = HashMap::new();
        for cmd in commands.iter() {
            cmds.insert(cmd.get_name(), cmd.get_description());
        }
        cmds.insert(
            "help".to_string(),
            "Get help for the given command\nUsage: help <command name>".to_string(),
        );
        HelpCommand { commands: cmds }
    }
}

impl Command for HelpCommand {
    fn get_name(&self) -> String {
        String::from("help")
    }

    fn get_aliases(&self) -> Vec<String> {
        vec![String::from("h")]
    }

    fn execute(&self, tx: Sender<AppEvent>, args: &Vec<String>) -> Result<(), CommandError> {
        if args.len() < 1 {
            return Err(CommandError::InvalidSyntax);
        }

        if self.commands.contains_key(&args[0]) {
            block_on(tx.send(AppEvent::ShowDialog((
                format!("Help for {} command", args[0]),
                if let Some(desc) = self.commands.get(&args[0]) {
                    desc.to_string()
                } else {
                    "No description provided :(".to_string()
                },
            ))));
        }

        Ok(())
    }

    fn get_description(&self) -> String {
        "Get help for the given command\nUsage: help <command name>".to_string()
    }
}

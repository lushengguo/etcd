use serde::{Deserialize, Serialize};
use crate::raft::node::Configuration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandType {
    Set,
    Get,
    Del,
    ConfigChange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub command_type: CommandType,
    pub key: String,
    pub value: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CommandError {
    InvalidCommand,
    MissingKey,
    MissingValue,
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CommandError::InvalidCommand => write!(f, "Invalid command"),
            CommandError::MissingKey => write!(f, "Missing key"),
            CommandError::MissingValue => write!(f, "Missing value"),
        }
    }
}

impl Command {
    pub fn new(command_type: CommandType, key: String, value: Option<String>) -> Self {
        Command {
            command_type,
            key,
            value,
        }
    }

    pub fn new_set(key: String, value: String) -> Self {
        Command {
            command_type: CommandType::Set,
            key,
            value: Some(value),
        }
    }

    pub fn new_get(key: String) -> Self {
        Command {
            command_type: CommandType::Get,
            key,
            value: None,
        }
    }

    pub fn new_del(key: String) -> Self {
        Command {
            command_type: CommandType::Del,
            key,
            value: None,
        }
    }

    pub fn new_config_change(old_config: Configuration, new_config: Configuration) -> Self {
        Command {
            command_type: CommandType::ConfigChange,
            key: "config".to_string(),
            value: Some(serde_json::to_string(&(old_config, new_config)).unwrap()),
        }
    }
}

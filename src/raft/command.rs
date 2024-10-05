#[derive(Debug, Clone)]
pub enum CommandType {
    Set,
    Get,
    Del,
}

#[derive(Debug, Clone)]
pub struct Command {
    pub command_type: CommandType,
    pub key: String,
    pub value: Option<String>,
}

#[derive(Debug)]
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
    pub fn new(command: String) -> Result<Self, CommandError> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        match parts.get(0) {
            Some(&"SET") => {
                if parts.len() < 3 {
                    return Err(CommandError::MissingValue);
                }
                Ok(Command::set(parts[1].to_string(), parts[2].to_string()))
            }
            Some(&"GET") => {
                if parts.len() < 2 {
                    return Err(CommandError::MissingKey);
                }
                Ok(Command::get(parts[1].to_string()))
            }
            Some(&"DEL") => {
                if parts.len() < 2 {
                    return Err(CommandError::MissingKey);
                }
                Ok(Command::del(parts[1].to_string()))
            }
            _ => Err(CommandError::InvalidCommand),
        }
    }

    pub fn set(key: String, value: String) -> Self {
        Command {
            command_type: CommandType::Set,
            key,
            value: Some(value),
        }
    }

    pub fn get(key: String) -> Self {
        Command {
            command_type: CommandType::Get,
            key,
            value: None,
        }
    }

    pub fn del(key: String) -> Self {
        Command {
            command_type: CommandType::Del,
            key,
            value: None,
        }
    }
}

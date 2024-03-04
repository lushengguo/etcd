pub enum Command {
    Insert,
    Update,
    Delete,
}

pub struct Operation {
    command: Command,
    key: String,
    value: String,
}

impl Operation {
    // raw_data format should be "command key value"
    // key value should be \w+ in regex
    pub fn new(raw_data: &Vec<u8>) -> Result<Operation, String> {
        let data = String::from_utf8(raw_data.to_vec()).unwrap();
        if data.matches(' ').count() != 2 {
            return Err("Invalid number of arguments".to_string());
        }
        let mut parts = data.split_whitespace();
        let command = match parts.next() {
            Some("Insert") => Command::Insert,
            Some("Update") => Command::Update,
            Some("Delete") => Command::Delete,
            _ => return Err("Invalid command".to_string()),
        };
        let key = match parts.next() {
            Some(key) => key.to_string(),
            None => return Err("Invalid key".to_string()),
        };
        let value = match parts.next() {
            Some(value) => value.to_string(),
            None => return Err("Invalid value".to_string()),
        };
        Ok(Operation {
            command,
            key,
            value,
        })
    }

    pub fn serialize_as_raw(&self) -> Vec<u8> {
        let mut result = String::new();
        match self.command {
            Command::Insert => result.push_str("Insert"),
            Command::Update => result.push_str("Update"),
            Command::Delete => result.push_str("Delete"),
        }
        result.push(' ');
        result.push_str(&self.key);
        result.push(' ');
        result.push_str(&self.value);
        result.into_bytes()
    }
}

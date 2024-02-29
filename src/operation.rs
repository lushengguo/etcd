pub enum Command {
    Insert = "Insert",
    Update = "Update",
    Delete = "Delete",
}

pub struct Operation {
    command: Command,
    key: String,
    value: String,
}

// raw_data format should be "command key value"
pub fn parse(raw_data: Vec<u8>) -> Result<Operation> {
    let data = String::from_utf8(raw_data).unwrap();
    if let whitespace_count = data.matches(' ').count() != 2 {
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

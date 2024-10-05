use super::command::Command;

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub term: u64,
    pub command: Option<Command>,
}

impl LogEntry {
    pub fn new(term: u64, command: Option<Command>) -> Self {
        LogEntry { term, command }
    }
}

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogEntry {
    pub term: u64,
    pub index: u64,
    pub command: String,
}

impl LogEntry {
    pub fn new(term: u64, index: u64, command: String) -> Self {
        LogEntry { term, index, command }
    }
}

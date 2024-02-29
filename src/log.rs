

struct Log {
    term: u64,
    index: u64,
    command: Command,
    key: String,
    value: String,
}

type Logs = Vec<Log>;


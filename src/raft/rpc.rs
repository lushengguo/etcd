use jsonrpc_core::Result;
use jsonrpc_derive::rpc;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LogEntry {
    pub term: u64,
    pub index: u64,
    pub command: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppendEntriesRequest {
    pub term: u64,
    pub leader_id: u64,
    pub prev_log_index: u64,
    pub prev_log_term: u64,
    pub entries: Vec<LogEntry>,
    pub leader_commit: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppendEntriesResponse {
    pub term: u64,
    pub success: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RequestVoteRequest {
    pub term: u64,
    pub candidate_id: u64,
    pub last_log_index: u64,
    pub last_log_term: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RequestVoteResponse {
    pub term: u64,
    pub vote_granted: bool,
}

#[rpc]
pub trait RaftRpc {
    #[rpc(name = "append_entries")]
    fn append_entries(&self, req: AppendEntriesRequest) -> Result<AppendEntriesResponse>;

    #[rpc(name = "request_vote")]
    fn request_vote(&self, req: RequestVoteRequest) -> Result<RequestVoteResponse>;
} 
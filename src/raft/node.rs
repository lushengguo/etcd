use raft_protobufs::raft_server::{Raft, RaftServer};
use raft_protobufs::{
    AppendEntriesRequest, AppendEntriesResponse, RequestVoteRequest, RequestVoteResponse,
};
use tonic::{transport::Server, Request, Response, Status};

use serde::Deserialize;
use std::collections::HashMap;

use super::log::LogEntry;
use super::state::State as NodeState;

pub mod raft_protobufs {
    tonic::include_proto!("raft_protobufs");
}

#[derive(Debug, Deserialize)]
pub struct RemoteNode {
    pub id: u64,
    pub address: String,
}

#[derive(Debug)]
pub struct LocalNode {
    map: HashMap<String, String>,

    id: u64,
    state: NodeState,

    // persistent state
    current_term: u64, /* latest term server has seen(initialized to 0 on first boot, increases
                        * monotonically) */
    voted_for: Option<u64>, // candidateId that received vote in current term (or null if none)
    log: Vec<LogEntry>,     /* log entries; each entry contains command for state machine, and
                             * term when entry was received by
                             * leader (first index is 1) */

    // volatile state
    commit_index: u64, /* index of highest log entry known to be committed (initialized to 0,
                        * increases monotonically) */
    last_applied: u64, /* index of highest log entry applied to state machine (initialized to 0,
                        * increases monotonically) */

    // leader volatile state (Reinitialized after election)
    next_index: Vec<u64>, /* for each server, index of the next log entry to send to that server
                           * (initialized to leader last log index +
                           * 1) */
    match_index: Vec<u64>, /* for each server, index of highest log entry known to be replicated
                            * on server (initialized to 0, increases
                            * monotonically) */
}

impl LocalNode {
    pub fn new(id: u64, other_nodes: Vec<RemoteNode>) -> Self {
        LocalNode {
            map: HashMap::new(),
            id,
            state: NodeState::Follower,
            current_term: 0,
            voted_for: None,
            log: vec![],
            commit_index: 0,
            last_applied: 0,
            next_index: vec![],
            match_index: vec![],
        }
    }

    pub fn set(&mut self, key: String, value: String) -> Status {
        Status::ok("")
    }

    pub fn get(&mut self, key: String) -> Result<String, Status> {
        Ok("".to_string())
    }

    pub fn del(&mut self, key: String) -> Status {
        Status::ok("")
    }

    pub fn handle_append_entries(&mut self, entries: Vec<LogEntry>) {
        // 处理 AppendEntries RPC
    }

    pub fn handle_request_vote(&mut self) -> bool {
        // 处理 RequestVote RPC
        false
    }
}

#[tonic::async_trait]
impl Raft for LocalNode {
    async fn append_entries(
        &self,
        request: Request<AppendEntriesRequest>,
    ) -> Result<Response<AppendEntriesResponse>, Status> {
        let req = request.into_inner();
        let reply = AppendEntriesResponse {
            term: 0,
            success: false,
        };
        Ok(Response::new(reply))
    }

    async fn request_vote(
        &self,
        request: Request<RequestVoteRequest>,
    ) -> Result<Response<RequestVoteResponse>, Status> {
        let req = request.into_inner();
        let reply = RequestVoteResponse {
            term: 0,
            vote_granted: false,
        };
        Ok(Response::new(reply))
    }
}

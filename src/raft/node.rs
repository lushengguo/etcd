use raft_protobufs::raft_server::{Raft, RaftServer};
use raft_protobufs::{Request as RaftRequest, Response as RaftResponse};
use tonic::{transport::Server, Request, Response, Status};

use serde::Deserialize;

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
    id: u64,
    state: NodeState,
    log: Vec<LogEntry>,
}

impl LocalNode {
    pub fn new(id: u64, other_nodes: Vec<RemoteNode>) -> Self {
        LocalNode {
            id,
            state: NodeState::Follower,
            log: Vec::new(),
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
    async fn hello_world(
        &self,
        request: Request<RaftRequest>,
    ) -> Result<Response<RaftResponse>, Status> {
        let req = request.into_inner();
        let reply = RaftResponse {
            message: format!("Hello, World!"),
        };
        Ok(Response::new(reply))
    }
}

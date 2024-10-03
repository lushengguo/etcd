use raft_proto::raft_server::{Raft, RaftServer};
use raft_proto::{Request as RaftRequest, Response as RaftResponse};
use tonic::{transport::Server, Request, Response, Status};

use super::log::LogEntry;
use super::state::State as NodeState;

pub mod raft_proto {
    tonic::include_proto!("raft_proto");
}

#[derive(Debug, Default)]
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

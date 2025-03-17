use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};
use log::debug;

use crate::raft::node::LocalNode;
use crate::raft::rpc::{
    AppendEntriesRequest as NodeAppendRequest, LogEntry, RequestVoteRequest as NodeRequestVote,
};
use crate::raft_proto::{
    raft_service_server::{RaftService, RaftServiceServer},
    AppendEntriesRequest, AppendEntriesResponse, LogEntry as ProtoLogEntry, RequestVoteRequest,
    RequestVoteResponse,
};

fn convert_log_entry(proto_entry: &ProtoLogEntry) -> LogEntry {
    LogEntry {
        term: proto_entry.term,
        index: proto_entry.index,
        command: proto_entry.command.clone(),
    }
}

fn convert_to_proto_log_entry(entry: &LogEntry) -> ProtoLogEntry {
    ProtoLogEntry {
        term: entry.term,
        index: entry.index,
        command: entry.command.clone(),
    }
}

pub struct RaftRpcService {
    node: Arc<Mutex<LocalNode>>,
}

impl RaftRpcService {
    pub fn new(node: Arc<Mutex<LocalNode>>) -> Self {
        Self { node }
    }

    pub fn server(self) -> RaftServiceServer<Self> {
        RaftServiceServer::new(self)
    }
}

#[tonic::async_trait]
impl RaftService for RaftRpcService {
    async fn append_entries(
        &self,
        request: Request<AppendEntriesRequest>,
    ) -> Result<Response<AppendEntriesResponse>, Status> {
        let req = request.into_inner();
        
        debug!(
            "RPC Server: Received AppendEntries from node {}: term={}, entries_count={}, prev_log_index={}, prev_log_term={}, leader_commit={}",
            req.leader_id, req.term, req.entries.len(), req.prev_log_index, req.prev_log_term, req.leader_commit
        );

        let node_request = NodeAppendRequest {
            term: req.term,
            leader_id: req.leader_id,
            prev_log_index: req.prev_log_index,
            prev_log_term: req.prev_log_term,
            entries: req.entries.iter().map(convert_log_entry).collect(),
            leader_commit: req.leader_commit,
        };

        let mut node_guard = self.node.lock().await;
        let node_id = node_guard.node_uid;
        let response = node_guard.handle_append_entries(node_request).await;
        
        debug!(
            "RPC Server: Node {} responding to AppendEntries from node {}: success={}, term={}",
            node_id, req.leader_id, response.success, response.term
        );

        Ok(Response::new(AppendEntriesResponse {
            term: response.term,
            success: response.success,
        }))
    }

    async fn request_vote(
        &self,
        request: Request<RequestVoteRequest>,
    ) -> Result<Response<RequestVoteResponse>, Status> {
        let req = request.into_inner();
        
        debug!(
            "RPC Server: Received RequestVote from node {}: term={}, last_log_index={}, last_log_term={}",
            req.candidate_id, req.term, req.last_log_index, req.last_log_term
        );

        let node_request = NodeRequestVote {
            term: req.term,
            candidate_id: req.candidate_id,
            last_log_index: req.last_log_index,
            last_log_term: req.last_log_term,
        };

        let mut node_guard = self.node.lock().await;
        let node_id = node_guard.node_uid;
        let response = node_guard.handle_request_vote(node_request).await;
        
        debug!(
            "RPC Server: Node {} responding to RequestVote from node {}: vote_granted={}, term={}",
            node_id, req.candidate_id, response.vote_granted, response.term
        );

        Ok(Response::new(RequestVoteResponse {
            term: response.term,
            vote_granted: response.vote_granted,
        }))
    }
}

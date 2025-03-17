use std::error::Error;
use tonic::transport::Channel;
use log::debug;

use crate::raft_proto::{
    raft_service_client::RaftServiceClient, AppendEntriesRequest, AppendEntriesResponse,
    LogEntry as ProtoLogEntry, RequestVoteRequest, RequestVoteResponse,
};

pub struct RaftClient {
    client: RaftServiceClient<Channel>,
    addr: String,
}

impl RaftClient {
    pub async fn connect(addr: &str) -> Result<Self, Box<dyn Error>> {
        let url = format!("http://{}", addr);
        debug!("RaftClient: Connecting to {}", url);
        let client = RaftServiceClient::connect(url.clone()).await?;
        debug!("RaftClient: Successfully connected to {}", url);
        Ok(Self { 
            client,
            addr: addr.to_string()
        })
    }

    pub async fn append_entries(
        &mut self,
        term: u64,
        leader_id: u64,
        prev_log_index: u64,
        prev_log_term: u64,
        entries: Vec<ProtoLogEntry>,
        leader_commit: u64,
    ) -> Result<AppendEntriesResponse, Box<dyn Error>> {
        debug!(
            "RaftClient: Sending AppendEntries to {}: term={}, leader_id={}, entries_count={}, prev_log_index={}, prev_log_term={}, leader_commit={}",
            self.addr, term, leader_id, entries.len(), prev_log_index, prev_log_term, leader_commit
        );
        
        let request = AppendEntriesRequest {
            term,
            leader_id,
            prev_log_index,
            prev_log_term,
            entries,
            leader_commit,
        };

        let response = self.client.append_entries(request).await?;
        let response_inner = response.into_inner();
        
        debug!(
            "RaftClient: Received AppendEntries response from {}: success={}, term={}",
            self.addr, response_inner.success, response_inner.term
        );
        
        Ok(response_inner)
    }

    pub async fn request_vote(
        &mut self,
        term: u64,
        candidate_id: u64,
        last_log_index: u64,
        last_log_term: u64,
    ) -> Result<RequestVoteResponse, Box<dyn Error>> {
        debug!(
            "RaftClient: Sending RequestVote to {}: term={}, candidate_id={}, last_log_index={}, last_log_term={}",
            self.addr, term, candidate_id, last_log_index, last_log_term
        );
        
        let request = RequestVoteRequest {
            term,
            candidate_id,
            last_log_index,
            last_log_term,
        };

        let response = self.client.request_vote(request).await?;
        let response_inner = response.into_inner();
        
        debug!(
            "RaftClient: Received RequestVote response from {}: vote_granted={}, term={}",
            self.addr, response_inner.vote_granted, response_inner.term
        );
        
        Ok(response_inner)
    }
}

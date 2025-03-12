use std::error::Error;
use tonic::transport::Channel;

use crate::raft_proto::{
    raft_service_client::RaftServiceClient, AppendEntriesRequest, AppendEntriesResponse,
    LogEntry as ProtoLogEntry, RequestVoteRequest, RequestVoteResponse,
};

pub struct RaftClient {
    client: RaftServiceClient<Channel>,
}

impl RaftClient {
    pub async fn connect(addr: &str) -> Result<Self, Box<dyn Error>> {
        let url = format!("http://{}", addr);
        let client = RaftServiceClient::connect(url).await?;
        Ok(Self { client })
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
        let request = AppendEntriesRequest {
            term,
            leader_id,
            prev_log_index,
            prev_log_term,
            entries,
            leader_commit,
        };

        let response = self.client.append_entries(request).await?;
        Ok(response.into_inner())
    }

    pub async fn request_vote(
        &mut self,
        term: u64,
        candidate_id: u64,
        last_log_index: u64,
        last_log_term: u64,
    ) -> Result<RequestVoteResponse, Box<dyn Error>> {
        let request = RequestVoteRequest {
            term,
            candidate_id,
            last_log_index,
            last_log_term,
        };

        let response = self.client.request_vote(request).await?;
        Ok(response.into_inner())
    }
}

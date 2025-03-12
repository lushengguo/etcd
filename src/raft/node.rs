use log::info;
use rand;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::Mutex;
use tonic::Status;

pub type RpcResult<T> = Result<T, Status>;

use crate::raft::rpc::{
    AppendEntriesRequest, AppendEntriesResponse, LogEntry, RaftRpc, RequestVoteRequest,
    RequestVoteResponse,
};

#[derive(Debug, Clone, PartialEq)]
pub enum NodeState {
    Follower,
    Candidate,
    Leader,
}

#[derive(Debug, Clone)]
pub struct RemoteNode {
    pub node_uid: u64,
    pub addr: String,
}

pub struct LocalNode {
    pub node_uid: u64,
    pub state: NodeState,
    pub current_term: u64,
    pub voted_for: Option<u64>,
    pub log: Vec<LogEntry>,
    pub commit_index: u64,
    pub last_applied: u64,
    pub next_index: HashMap<u64, u64>,
    pub match_index: HashMap<u64, u64>,
    
    pub client_to_cluster: HashMap<u64, String>, 
    pub last_heartbeat: SystemTime,
    pub election_timeout: u64,
    pub heartbeat_interval: u64,
    pub kv_store: HashMap<String, String>,
}

impl LocalNode {
    pub fn new(node_uid: u64) -> Self {
        let election_timeout = rand::random::<u64>() % 150 + 150;

        Self {
            node_uid,
            state: NodeState::Follower,
            current_term: 0,
            voted_for: None,
            log: Vec::new(),
            commit_index: 0,
            last_applied: 0,
            next_index: HashMap::new(),
            match_index: HashMap::new(),
            client_to_cluster: HashMap::new(),
            last_heartbeat: SystemTime::now(),
            election_timeout,
            heartbeat_interval: 50,
            kv_store: HashMap::new(),
        }
    }

    pub async fn connect_to_cluster(
        &mut self,
        nodes: Vec<RemoteNode>,
    ) -> Result<(), Box<dyn Error>> {
        for node in nodes {
            self.client_to_cluster
                .insert(node.node_uid, node.addr.clone());
            info!("Added node {} at {}", node.node_uid, node.addr);
        }
        Ok(())
    }
    
    pub async fn set(&mut self, key: String, value: String) -> RpcResult<()> {
        if self.state != NodeState::Leader {
            return Err(Status::invalid_argument("Invalid state"));
        }
        
        let log_entry = LogEntry {
            term: self.current_term,
            index: self.log.len() as u64 + 1,
            command: format!("SET {} {}", key, value),
        };
        
        self.log.push(log_entry.clone());
        
        if self.replicate_log().await {
            self.kv_store.insert(key, value);
            Ok(())
        } else {
            Err(Status::internal("Failed to replicate log"))
        }
    }
    
    pub async fn get(&self, key: String) -> RpcResult<String> {
        match self.kv_store.get(&key) {
            Some(value) => Ok(value.clone()),
            None => Err(Status::not_found("Key not found")),
        }
    }
    
    pub async fn delete(&mut self, key: String) -> RpcResult<()> {
        if self.state != NodeState::Leader {
            return Err(Status::invalid_argument("Not a leader"));
        }
        
        let log_entry = LogEntry {
            term: self.current_term,
            index: self.log.len() as u64 + 1,
            command: format!("DEL {}", key),
        };
        
        self.log.push(log_entry.clone());
        
        if self.replicate_log().await {
            self.kv_store.remove(&key);
            Ok(())
        } else {
            Err(Status::internal("Failed to replicate log"))
        }
    }

    pub async fn send_heartbeats(&mut self) -> bool {
        if self.state != NodeState::Leader {
            return false;
        }

        let mut success_count = 1;

        for (node_uid, addr) in self.client_to_cluster.clone() {
            if node_uid == self.node_uid {
                continue;
            }

            match self.send_append_entries(node_uid, addr, vec![]).await {
                Ok(success) => {
                    if success {
                        success_count += 1;
                    }
                }
                Err(e) => {
                    info!(
                        "Failed to send heartbeat to node {}: {:?}",
                        node_uid, e
                    );
                }
            }
        }

        let majority = (self.client_to_cluster.len() / 2) + 1;
        success_count >= majority
    }

    pub async fn send_append_entries(
        &mut self,
        target_node_id: u64,
        target_addr: String,
        entries: Vec<LogEntry>,
    ) -> Result<bool, Box<dyn Error>> {
        use crate::raft::raft_client::RaftClient;
        use crate::raft_proto::LogEntry as ProtoLogEntry;

        let proto_entries: Vec<ProtoLogEntry> = entries
            .iter()
            .map(|entry| ProtoLogEntry {
                term: entry.term,
                index: entry.index,
                command: entry.command.clone(),
            })
            .collect();

        let prev_log_index = if self.log.is_empty() {
            0
        } else {
            self.log.len() as u64
        };
        let prev_log_term = if self.log.is_empty() {
            0
        } else {
            self.log.last().unwrap().term
        };

        let mut client = RaftClient::connect(&target_addr).await?;
        let response = client
            .append_entries(
                self.current_term,
                self.node_uid,
                prev_log_index,
                prev_log_term,
                proto_entries,
                self.commit_index,
            )
            .await?;

        if response.term > self.current_term {
            self.current_term = response.term;
            self.state = NodeState::Follower;
            self.voted_for = None;
            return Ok(false);
        }

        Ok(response.success)
    }

    pub async fn send_request_vote(
        &mut self,
        target_node_id: u64,
        target_addr: String,
    ) -> Result<bool, Box<dyn Error>> {
        use crate::raft::raft_client::RaftClient;

        let last_log_index = if self.log.is_empty() {
            0
        } else {
            self.log.len() as u64
        };
        let last_log_term = if self.log.is_empty() {
            0
        } else {
            self.log.last().unwrap().term
        };

        let mut client = RaftClient::connect(&target_addr).await?;
        let response = client
            .request_vote(
                self.current_term,
                self.node_uid,
                last_log_index,
                last_log_term,
            )
            .await?;

        if response.term > self.current_term {
            self.current_term = response.term;
            self.state = NodeState::Follower;
            self.voted_for = None;
            return Ok(false);
        }

        Ok(response.vote_granted)
    }

    pub async fn replicate_log(&mut self) -> bool {
        if self.state != NodeState::Leader {
            return false;
        }

        let mut success_count = 1;

        let clients = self.client_to_cluster.clone();
        let node_id = self.node_uid;

        for (node_uid, addr) in clients {
            if node_uid == node_id {
                continue;
            }

            let next_idx = self.next_index.get(&node_uid).cloned().unwrap_or(1);
            if next_idx <= self.log.len() as u64 {
                let entries_to_send = self.log[(next_idx - 1) as usize..].to_vec();

                match self
                    .send_append_entries(node_uid, addr, entries_to_send)
                    .await
                {
                    Ok(success) => {
                        if success {
                            let match_idx = self.log.len() as u64;
                            self.next_index.insert(node_uid, match_idx + 1);
                            self.match_index.insert(node_uid, match_idx);
                            success_count += 1;
                        } else {
                            let new_next_idx = next_idx.saturating_sub(1);
                            self.next_index.insert(node_uid, new_next_idx);
                        }
                    }
                    Err(e) => {
                        info!(
                            "Failed to replicate log to node {}: {:?}",
                            node_uid, e
                        );
                    }
                }
            } else {
                success_count += 1;
            }
        }

        let majority = (self.client_to_cluster.len() / 2) + 1;
        success_count >= majority
    }

    pub fn is_election_timeout(&self) -> bool {
        if self.state == NodeState::Leader {
            return false;
        }

        match SystemTime::now().duration_since(self.last_heartbeat) {
            Ok(duration) => duration.as_millis() as u64 > self.election_timeout,
            Err(_) => false,
        }
    }

    pub async fn start_election(&mut self) -> bool {
        info!(
            "Node {} starting election, term {}",
            self.node_uid, self.current_term
        );

        self.state = NodeState::Candidate;
        self.current_term += 1;
        self.voted_for = Some(self.node_uid);
        self.last_heartbeat = SystemTime::now();

        let mut votes_received = 1;

        let clients = self.client_to_cluster.clone();
        let node_id = self.node_uid;

        for (node_uid, addr) in clients {
            if node_uid == node_id {
                continue;
            }

            match self.send_request_vote(node_uid, addr).await {
                Ok(granted) => {
                    if granted {
                        votes_received += 1;
                    }
                }
                Err(e) => {
                    info!(
                        "Failed to send vote request to node {}: {:?}",
                        node_uid, e
                    );
                }
            }

            if self.state != NodeState::Candidate {
                return false;
            }
        }

        let majority = (self.client_to_cluster.len() / 2) + 1;
        let won_election = votes_received >= majority;

        if won_election {
            info!(
                "Node {} won election, term {}",
                self.node_uid, self.current_term
            );

            self.state = NodeState::Leader;

            for node_id in self.client_to_cluster.keys() {
                if *node_id != self.node_uid {
                    self.next_index.insert(*node_id, self.log.len() as u64 + 1);
                    self.match_index.insert(*node_id, 0);
                }
            }

            let _ = self.send_heartbeats().await;
        } else {
            self.state = NodeState::Follower;
        }

        won_election
    }

    pub async fn handle_heartbeat_timeout(&mut self) {
        match self.state {
            NodeState::Leader => {
                let _ = self.send_heartbeats().await;
            }
            _ => {
                if self.is_election_timeout() {
                    let _ = self.start_election().await;
                }
            }
        }
    }
    
    pub async fn handle_append_entries(
        &mut self,
        req: AppendEntriesRequest,
    ) -> AppendEntriesResponse {
        self.last_heartbeat = SystemTime::now();

        if req.term < self.current_term {
            return AppendEntriesResponse {
                term: self.current_term,
                success: false,
            };
        }

        if req.term > self.current_term {
            self.current_term = req.term;
            self.state = NodeState::Follower;
            self.voted_for = None;
        }

        if !req.entries.is_empty() {}

        if req.leader_commit > self.commit_index {
            self.commit_index = req.leader_commit.min(self.log.len() as u64);
        }
        
        AppendEntriesResponse {
            term: self.current_term,
            success: true,
        }
    }

    pub async fn handle_request_vote(&mut self, req: RequestVoteRequest) -> RequestVoteResponse {
        if req.term < self.current_term {
            return RequestVoteResponse {
                term: self.current_term,
                vote_granted: false,
            };
        }

        if req.term > self.current_term {
            self.current_term = req.term;
            self.state = NodeState::Follower;
            self.voted_for = None;
        }

        let can_vote = self.voted_for.is_none() || self.voted_for == Some(req.candidate_id);

        let log_is_current = self.log.is_empty()
            || req.last_log_term > self.log.last().unwrap().term
            || (req.last_log_term == self.log.last().unwrap().term
                && req.last_log_index >= self.log.len() as u64);

        if can_vote && log_is_current {
            self.voted_for = Some(req.candidate_id);
            self.last_heartbeat = SystemTime::now();

            return RequestVoteResponse {
                term: self.current_term,
                vote_granted: true,
            };
        }
        
        RequestVoteResponse {
            term: self.current_term,
            vote_granted: false,
        }
    }
}

pub struct RaftRpcImpl {
    node: Arc<Mutex<LocalNode>>,
}

impl RaftRpcImpl {
    pub fn new(node: Arc<Mutex<LocalNode>>) -> Self {
        Self { node }
    }
}

impl RaftRpc for RaftRpcImpl {
    fn append_entries(&self, _req: AppendEntriesRequest) -> RpcResult<AppendEntriesResponse> {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let node = self.node.clone();
        
        let response = rt.block_on(async move {
            let mut node = node.lock().await;
            node.handle_append_entries(_req).await
        });
        
        Ok(response)
    }

    fn request_vote(&self, _req: RequestVoteRequest) -> RpcResult<RequestVoteResponse> {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let node = self.node.clone();
        
        let response = rt.block_on(async move {
            let mut node = node.lock().await;
            node.handle_request_vote(_req).await
        });
        
        Ok(response)
    }
}

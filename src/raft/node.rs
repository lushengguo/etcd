use log::{debug, info};
use rand;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::Mutex;
use tonic::Status;
use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

use crate::raft::command::Command;
use crate::raft::log::LogEntry;
use crate::raft::rpc::{
    AppendEntriesRequest, AppendEntriesResponse, RaftRpc, RequestVoteRequest,
    RequestVoteResponse,
};

pub type RpcResult<T> = Result<T, Status>;

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

#[derive(Serialize, Deserialize)]
struct PersistentState {
    current_term: u64,
    voted_for: Option<u64>,
    log: Vec<LogEntry>,
}

#[derive(Serialize, Deserialize)]
pub struct Snapshot {
    last_included_index: u64,
    last_included_term: u64,
    data: Vec<u8>,
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
    pub configuration_state: Option<ConfigurationState>,
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
            configuration_state: None,
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
            debug!("Node {} cannot send heartbeats: not a leader (state: {:?})", self.node_uid, self.state);
            return false;
        }

        debug!(
            "Node {} (Leader, term {}) sending heartbeats to {} nodes",
            self.node_uid, self.current_term, self.client_to_cluster.len() - 1
        );

        let mut success_count = 1; // Count self as success
        let clients = self.client_to_cluster.clone();
        let node_id = self.node_uid;

        for (target_node_id, target_addr) in clients {
            if target_node_id == node_id {
                continue;
            }

            let prev_log_index = self.next_index.get(&target_node_id).unwrap_or(&1).saturating_sub(1);
            let prev_log_term = if prev_log_index == 0 {
                0
            } else if prev_log_index <= self.log.len() as u64 {
                self.log[(prev_log_index - 1) as usize].term
            } else {
                debug!(
                    "Node {} invalid prev_log_index {} for node {}, log length is {}",
                    self.node_uid, prev_log_index, target_node_id, self.log.len()
                );
                continue;
            };

            let entries = if let Some(&next_idx) = self.next_index.get(&target_node_id) {
                if next_idx <= self.log.len() as u64 {
                    self.log[(next_idx - 1) as usize..].to_vec()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };

            debug!(
                "Node {} sending heartbeat to node {}: prev_log_index={}, prev_log_term={}, entries_count={}",
                self.node_uid, target_node_id, prev_log_index, prev_log_term, entries.len()
            );

            // 添加超时处理
            match tokio::time::timeout(
                tokio::time::Duration::from_millis(500),
                self.send_append_entries(
                    target_node_id,
                    target_addr,
                    prev_log_index,
                    prev_log_term,
                    entries,
                )
            ).await {
                Ok(Ok(success)) => {
                    if success {
                        success_count += 1;
                        debug!(
                            "Node {} received successful heartbeat response from node {}",
                            self.node_uid, target_node_id
                        );
                    } else {
                        debug!(
                            "Node {} received failed heartbeat response from node {}",
                            self.node_uid, target_node_id
                        );
                    }
                }
                Ok(Err(e)) => {
                    info!(
                        "Node {} failed to send heartbeat to node {}: {:?}",
                        self.node_uid, target_node_id, e
                    );
                }
                Err(_) => {
                    info!(
                        "Node {} heartbeat to node {} timed out",
                        self.node_uid, target_node_id
                    );
                }
            }
        }

        let majority = (self.client_to_cluster.len() / 2) + 1;
        let result = success_count >= majority;

        debug!(
            "Node {} heartbeat result: success_count={}/{}, majority_needed={}, success={}",
            self.node_uid, success_count, self.client_to_cluster.len(), majority, result
        );

        result
    }

    pub async fn send_append_entries(
        &mut self,
        target_node_id: u64,
        target_addr: String,
        prev_log_index: u64,
        prev_log_term: u64,
        entries: Vec<LogEntry>,
    ) -> Result<bool, Box<dyn Error>> {
        use crate::raft::raft_client::RaftClient;
        use crate::raft_proto::LogEntry as ProtoLogEntry;

        let entries_count = entries.len();
        debug!(
            "Node {} sending AppendEntries to node {}: term={}, entries_count={}",
            self.node_uid, target_node_id, self.current_term, entries_count
        );

        let proto_entries: Vec<ProtoLogEntry> = entries
            .iter()
            .map(|entry| ProtoLogEntry {
                term: entry.term,
                index: entry.index,
                command: entry.command.clone(),
            })
            .collect();

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
            info!(
                "Node {} received higher term {} from node {} (current term: {}), reverting to follower",
                self.node_uid, response.term, target_node_id, self.current_term
            );
            self.current_term = response.term;
            self.state = NodeState::Follower;
            self.voted_for = None;
            return Ok(false);
        }

        debug!(
            "Node {} received AppendEntries response from node {}: success={}, term={}",
            self.node_uid, target_node_id, response.success, response.term
        );
        
        Ok(response.success)
    }

    pub async fn send_request_vote(
        &mut self,
        target_node_id: u64,
        target_addr: String,
    ) -> Result<bool, Box<dyn Error>> {
        use crate::raft::raft_client::RaftClient;

        debug!(
            "Node {} sending RequestVote to node {}: term={}",
            self.node_uid, target_node_id, self.current_term
        );

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

        debug!(
            "Node {} RequestVote details: last_log_index={}, last_log_term={}",
            self.node_uid, last_log_index, last_log_term
        );

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
            info!(
                "Node {} received higher term {} from node {} (current term: {}), reverting to follower",
                self.node_uid, response.term, target_node_id, self.current_term
            );
            self.current_term = response.term;
            self.state = NodeState::Follower;
            self.voted_for = None;
            return Ok(false);
        }

        debug!(
            "Node {} received RequestVote response from node {}: vote_granted={}, term={}",
            self.node_uid, target_node_id, response.vote_granted, response.term
        );

        Ok(response.vote_granted)
    }

    pub async fn replicate_log(&mut self) -> bool {
        if self.state != NodeState::Leader {
            return false;
        }

        let mut success_count = 1; // Count self as success
        let clients = self.client_to_cluster.clone();
        let node_id = self.node_uid;

        for (node_uid, addr) in clients {
            if node_uid == node_id {
                continue;
            }

            let next_idx = *self.next_index.get(&node_uid).unwrap_or(&1);
            if next_idx <= self.log.len() as u64 {
                let entries_to_send = self.log[(next_idx - 1) as usize..].to_vec();
                let entries_len = entries_to_send.len() as u64;
                let prev_log_index = next_idx - 1;
                let prev_log_term = if prev_log_index == 0 {
                    0
                } else {
                    self.log[(prev_log_index - 1) as usize].term
                };

                debug!(
                    "Node {} replicating {} entries to node {} (next_index={})",
                    self.node_uid,
                    entries_len,
                    node_uid,
                    next_idx
                );

                match self
                    .send_append_entries(node_uid, addr, prev_log_index, prev_log_term, entries_to_send)
                    .await
                {
                    Ok(success) => {
                        if success {
                            success_count += 1;
                            let new_next_idx = next_idx + entries_len;
                            self.next_index.insert(node_uid, new_next_idx);
                            self.match_index.insert(node_uid, new_next_idx - 1);
                            
                            debug!(
                                "Node {} successfully replicated to node {}: next_index {} -> {}",
                                self.node_uid, node_uid, next_idx, new_next_idx
                            );
                        } else {
                            let old_next_idx = next_idx;
                            let new_next_idx = next_idx.saturating_sub(1);
                            
                            self.next_index.insert(node_uid, new_next_idx);
                            
                            debug!(
                                "Node {} failed to replicate to node {}: decrementing next_index {} -> {}",
                                self.node_uid, node_uid, old_next_idx, new_next_idx
                            );
                        }
                    }
                    Err(e) => {
                        info!(
                            "Node {} failed to replicate log to node {}: {:?}",
                            self.node_uid, node_uid, e
                        );
                    }
                }
            } else {
                debug!(
                    "Node {} : No new entries to send to node {} (next_index={}, log_length={})",
                    self.node_uid, node_uid, next_idx, self.log.len()
                );
                success_count += 1;
            }
        }

        let majority = (self.client_to_cluster.len() / 2) + 1;
        let result = success_count >= majority;
        
        debug!(
            "Node {} log replication result: success_count={}/{}, majority_needed={}, success={}",
            self.node_uid, success_count, self.client_to_cluster.len(), majority, result
        );
        
        result
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
            "Node {} starting election for term {} (current term: {})",
            self.node_uid, self.current_term + 1, self.current_term
        );

        self.state = NodeState::Candidate;
        self.current_term += 1;
        self.voted_for = Some(self.node_uid);
        self.last_heartbeat = SystemTime::now();

        debug!(
            "Node {} became candidate for term {}, voting for self",
            self.node_uid, self.current_term
        );

        let mut votes_received = 1;
        let mut nodes_contacted = 0;

        let clients = self.client_to_cluster.clone();
        let node_id = self.node_uid;
        let total_nodes = clients.len();

        debug!(
            "Node {} election: total cluster size is {} nodes",
            self.node_uid, total_nodes
        );

        for (node_uid, addr) in clients {
            if node_uid == node_id {
                continue;
            }

            nodes_contacted += 1;
            debug!(
                "Node {} requesting vote from node {} for term {}",
                self.node_uid, node_uid, self.current_term
            );

            match tokio::time::timeout(
                tokio::time::Duration::from_millis(500),
                self.send_request_vote(node_uid, addr)
            ).await {
                Ok(Ok(granted)) => {
                    if granted {
                        votes_received += 1;
                        debug!(
                            "Node {} received vote from node {}, total votes: {}/{}",
                            self.node_uid, node_uid, votes_received, total_nodes
                        );
                    } else {
                        debug!(
                            "Node {} vote denied by node {}, total votes: {}/{}",
                            self.node_uid, node_uid, votes_received, total_nodes
                        );
                    }
                }
                Ok(Err(e)) => {
                    info!(
                        "Node {} failed to send vote request to node {}: {:?}",
                        self.node_uid, node_uid, e
                    );
                }
                Err(_) => {
                    info!(
                        "Node {} vote request to node {} timed out",
                        self.node_uid, node_uid
                    );
                }
            }

            if self.state != NodeState::Candidate {
                debug!(
                    "Node {} no longer a candidate (state: {:?}), aborting election for term {}",
                    self.node_uid, self.state, self.current_term
                );
                return false;
            }
        }

        let majority = (total_nodes / 2) + 1;
        let won_election = votes_received >= majority;

        if won_election {
            info!(
                "Node {} won election for term {}: received {} votes out of {} nodes (majority needed: {})",
                self.node_uid, self.current_term, votes_received, total_nodes, majority
            );

            self.state = NodeState::Leader;

            for node_id in self.client_to_cluster.keys() {
                if *node_id != self.node_uid {
                    let next_index = self.log.len() as u64 + 1;
                    debug!(
                        "Node {} initializing nextIndex for node {} to {} after winning election",
                        self.node_uid, node_id, next_index
                    );
                    self.next_index.insert(*node_id, next_index);
                    self.match_index.insert(*node_id, 0);
                }
            }

            debug!("Node {} sending initial heartbeats as new leader", self.node_uid);
            let result = self.send_heartbeats().await;
            debug!("Node {} initial heartbeats result: {}", self.node_uid, result);
        } else {
            info!(
                "Node {} lost election for term {}: received {} votes out of {} nodes (majority needed: {})",
                self.node_uid, self.current_term, votes_received, total_nodes, majority
            );
            self.state = NodeState::Follower;
        }

        won_election
    }

    pub async fn handle_heartbeat_timeout(&mut self) {
        match self.state {
            NodeState::Leader => {
                debug!(
                    "Node {} (Leader, term {}) sending periodic heartbeats",
                    self.node_uid, self.current_term
                );
                let result = self.send_heartbeats().await;
                if !result {
                    debug!(
                        "Node {} (Leader, term {}) failed to reach majority with heartbeats",
                        self.node_uid, self.current_term
                    );
                }
            }
            _ => {
                // Check if it's a single-node cluster (only itself)
                if self.client_to_cluster.len() == 1 && self.client_to_cluster.contains_key(&self.node_uid) {
                    // If it's a single-node cluster, directly become the leader
                    if self.state != NodeState::Leader {
                        info!(
                            "Node {} is the only node in cluster, becoming leader for term {}",
                            self.node_uid, self.current_term + 1
                        );
                        self.state = NodeState::Leader;
                        self.current_term += 1;
                        self.voted_for = Some(self.node_uid);
                        self.last_heartbeat = SystemTime::now();
                    }
                } else if self.is_election_timeout() {
                    let elapsed = match SystemTime::now().duration_since(self.last_heartbeat) {
                        Ok(duration) => duration.as_millis(),
                        Err(_) => 0,
                    };
                    
                    info!(
                        "Node {} detected election timeout after {} ms (timeout: {} ms), current state: {:?}, term: {}",
                        self.node_uid, elapsed, self.election_timeout, self.state, self.current_term
                    );
                    
                    let won = self.start_election().await;
                    
                    debug!(
                        "Node {} election result: {}, new state: {:?}, term: {}",
                        self.node_uid, won, self.state, self.current_term
                    );
                }
            }
        }
    }
    
    pub async fn handle_append_entries(
        &mut self,
        req: AppendEntriesRequest,
    ) -> AppendEntriesResponse {
        debug!(
            "Node {} received AppendEntries from node {}: term={}, entries_count={}, prev_log_index={}, prev_log_term={}, leader_commit={}",
            self.node_uid, req.leader_id, req.term, req.entries.len(), req.prev_log_index, req.prev_log_term, req.leader_commit
        );

        self.last_heartbeat = SystemTime::now();

        if req.term < self.current_term {
            debug!(
                "Node {} rejected AppendEntries from node {}: req.term {} < current term {}",
                self.node_uid, req.leader_id, req.term, self.current_term
            );
            return AppendEntriesResponse {
                term: self.current_term,
                success: false,
            };
        }

        if req.term > self.current_term {
            info!(
                "Node {} updating term: {} -> {} (AppendEntries from node {})",
                self.node_uid, self.current_term, req.term, req.leader_id
            );
            self.current_term = req.term;
            self.state = NodeState::Follower;
            self.voted_for = None;
        }

        if !req.entries.is_empty() {
            debug!(
                "Node {} processing {} log entries from leader {}",
                self.node_uid, req.entries.len(), req.leader_id
            );
            // Here should be code to process log entries
        }

        if req.leader_commit > self.commit_index {
            let old_commit_index = self.commit_index;
            self.commit_index = req.leader_commit.min(self.log.len() as u64);
            debug!(
                "Node {} updating commit index: {} -> {}",
                self.node_uid, old_commit_index, self.commit_index
            );
        }
        
        debug!(
            "Node {} accepted AppendEntries from node {}",
            self.node_uid, req.leader_id
        );
        
        AppendEntriesResponse {
            term: self.current_term,
            success: true,
        }
    }

    pub async fn handle_request_vote(&mut self, req: RequestVoteRequest) -> RequestVoteResponse {
        debug!(
            "Node {} received RequestVote from node {}: term={}, last_log_index={}, last_log_term={}",
            self.node_uid, req.candidate_id, req.term, req.last_log_index, req.last_log_term
        );

        if req.term < self.current_term {
            debug!(
                "Node {} rejected vote for node {}: req.term {} < current term {}",
                self.node_uid, req.candidate_id, req.term, self.current_term
            );
            return RequestVoteResponse {
                term: self.current_term,
                vote_granted: false,
            };
        }

        if req.term > self.current_term {
            info!(
                "Node {} updating term: {} -> {} (RequestVote from node {})",
                self.node_uid, self.current_term, req.term, req.candidate_id
            );
            self.current_term = req.term;
            self.state = NodeState::Follower;
            self.voted_for = None;
        }

        let can_vote = self.voted_for.is_none() || self.voted_for == Some(req.candidate_id);
        
        // 改进日志比较逻辑
        let my_last_log_term = self.log.last().map_or(0, |entry| entry.term);
        let my_last_log_index = self.log.len() as u64;
        
        let log_is_current = if req.last_log_term != my_last_log_term {
            // 如果任期不同，更高任期的日志更新
            req.last_log_term > my_last_log_term
        } else {
            // 如果任期相同，更长的日志更新
            req.last_log_index >= my_last_log_index
        };

        debug!(
            "Node {} vote decision factors: can_vote={}, log_is_current={}, my_last_term={}, my_last_index={}, req_last_term={}, req_last_index={}",
            self.node_uid, can_vote, log_is_current, my_last_log_term, my_last_log_index, req.last_log_term, req.last_log_index
        );

        if can_vote && log_is_current {
            info!(
                "Node {} voting for node {} for term {}",
                self.node_uid, req.candidate_id, req.term
            );
            self.voted_for = Some(req.candidate_id);
            self.last_heartbeat = SystemTime::now();

            return RequestVoteResponse {
                term: self.current_term,
                vote_granted: true,
            };
        }
        
        debug!(
            "Node {} rejected vote for node {}: can_vote={}, log_is_current={}",
            self.node_uid, req.candidate_id, can_vote, log_is_current
        );
        
        RequestVoteResponse {
            term: self.current_term,
            vote_granted: false,
        }
    }

    fn get_storage_path(&self) -> PathBuf {
        PathBuf::from(format!("raft_state_{}.json", self.node_uid))
    }

    pub fn save_state(&self) -> Result<(), Box<dyn Error>> {
        let state = PersistentState {
            current_term: self.current_term,
            voted_for: self.voted_for,
            log: self.log.clone(),
        };
        
        let json = serde_json::to_string(&state)?;
        fs::write(self.get_storage_path(), json)?;
        Ok(())
    }

    pub fn load_state(&mut self) -> Result<(), Box<dyn Error>> {
        let path = self.get_storage_path();
        if path.exists() {
            let json = fs::read_to_string(path)?;
            let state: PersistentState = serde_json::from_str(&json)?;
            
            self.current_term = state.current_term;
            self.voted_for = state.voted_for;
            self.log = state.log;
        }
        Ok(())
    }

    pub async fn update_term(&mut self, new_term: u64) -> Result<(), Box<dyn Error>> {
        if new_term > self.current_term {
            self.current_term = new_term;
            self.voted_for = None;
            self.state = NodeState::Follower;
            self.save_state()?;
        }
        Ok(())
    }

    pub async fn create_snapshot(&mut self) -> Result<(), Box<dyn Error>> {
        if self.log.is_empty() {
            return Ok(());
        }

        let last_index = self.commit_index;
        if last_index == 0 {
            return Ok(());
        }

        // Serialize the current state machine state
        let state_machine_data = serde_json::to_vec(&self.kv_store)?;
        
        let snapshot = Snapshot {
            last_included_index: last_index,
            last_included_term: self.log[last_index as usize].term,
            data: state_machine_data,
        };

        // Persist snapshot
        let snapshot_path = format!("snapshot_{}.json", self.node_uid);
        let json = serde_json::to_string(&snapshot)?;
        fs::write(snapshot_path, json)?;

        // Compress logs
        self.log.drain(0..=last_index as usize);
        
        // Update indexes
        self.last_applied = last_index;
        self.commit_index = last_index;

        Ok(())
    }

    pub async fn install_snapshot(&mut self, snapshot: Snapshot) -> Result<(), Box<dyn Error>> {
        if snapshot.last_included_index <= self.commit_index {
            return Ok(());
        }

        // Restore state machine state
        self.kv_store = serde_json::from_slice(&snapshot.data)?;

        // Update logs
        self.log.clear();
        self.log.push(LogEntry::new(
            snapshot.last_included_term,
            snapshot.last_included_index,
            String::new(),
        ));

        // Update indexes
        self.last_applied = snapshot.last_included_index;
        self.commit_index = snapshot.last_included_index;

        Ok(())
    }

    // Check if snapshot creation is needed
    pub async fn check_snapshot_needed(&mut self) -> Result<(), Box<dyn Error>> {
        const SNAPSHOT_THRESHOLD: usize = 1000; // Configurable threshold
        if self.log.len() > SNAPSHOT_THRESHOLD {
            self.create_snapshot().await?;
        }
        Ok(())
    }

    pub async fn change_configuration(&mut self, new_members: HashMap<u64, String>) -> Result<(), Box<dyn Error>> {
        // Ensure it's the Leader
        if self.state != NodeState::Leader {
            return Err("Only leader can change configuration".into());
        }

        // Create new configuration
        let old_config = Configuration {
            members: self.client_to_cluster.clone(),
        };
        let new_config = Configuration {
            members: new_members.clone(),
        };

        // Enter Joint Consensus phase
        self.configuration_state = Some(ConfigurationState::Joint(old_config.clone(), new_config.clone()));

        // Create configuration change log entry
        let config_command = Command::new_config_change(old_config, new_config.clone());
        let config_entry = LogEntry::new(
            self.current_term,
            self.log.len() as u64,
            serde_json::to_string(&config_command)?,
        );
        self.log.push(config_entry);

        // Wait for log replication
        let log_index = (self.log.len() - 1) as u64;
        self.replicate_log().await;

        // Wait for log commit
        while self.commit_index < log_index {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        // Switch to new configuration
        self.client_to_cluster = new_members;
        let config_state = ConfigurationState::Stable(new_config);
        self.configuration_state = Some(config_state);

        // Update next_index and match_index
        self.next_index.clear();
        self.match_index.clear();
        for &node_id in self.client_to_cluster.keys() {
            self.next_index.insert(node_id, self.log.len() as u64);
            self.match_index.insert(node_id, 0);
        }

        Ok(())
    }

    // Handle configuration change log entries
    async fn apply_config_change(&mut self, old_config: Configuration, new_config: Configuration) {
        // Enter Joint Consensus phase
        self.configuration_state = Some(ConfigurationState::Joint(old_config, new_config.clone()));
        
        // Apply new configuration
        self.client_to_cluster = new_config.members.clone();
        self.configuration_state = Some(ConfigurationState::Stable(new_config));
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Configuration {
    pub members: HashMap<u64, String>,
}

#[derive(Clone, Debug)]
pub enum ConfigurationState {
    Stable(Configuration),
    Joint(Configuration, Configuration),
}

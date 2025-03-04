use std::collections::HashMap;
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use log::info;
use std::sync::Arc;
use tokio::sync::Mutex;
use jsonrpc_core::Result as RpcResult;
use jsonrpc_core_client::TypedClient;
use jsonrpc_core_client::transports::http;


use crate::raft::rpc::{
    RaftRpc, AppendEntriesRequest, AppendEntriesResponse,
    RequestVoteRequest, RequestVoteResponse, LogEntry
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
}

impl LocalNode {
    pub fn new(node_uid: u64) -> Self {
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
        }
    }

    
    pub async fn connect_to_cluster(&mut self, nodes: Vec<RemoteNode>) -> Result<(), Box<dyn Error>> {
        for node in nodes {
            
            self.client_to_cluster.insert(node.node_uid, node.addr.clone());
            info!("Added node {} at {}", node.node_uid, node.addr);
        }
        Ok(())
    }

    
    pub async fn set(&mut self, key: String, value: String) -> RpcResult<()> {
        
        if self.state != NodeState::Leader {
            return Err(jsonrpc_core::Error::invalid_request());
        }

        
        let log_entry = LogEntry {
            term: self.current_term,
            index: self.log.len() as u64 + 1,
            command: format!("SET {} {}", key, value),
        };

        
        self.log.push(log_entry.clone());

        
        self.replicate_log().await;

        Ok(())
    }

    
    pub async fn get(&self, key: String) -> RpcResult<String> {
        
        Ok(String::new())
    }

    
    async fn replicate_log(&self) -> bool {
        
        true
    }

    
    pub async fn handle_append_entries(
        &mut self,
        _req: AppendEntriesRequest,
    ) -> AppendEntriesResponse {
        
        AppendEntriesResponse {
            term: self.current_term,
            success: true,
        }
    }

    
    pub async fn handle_request_vote(
        &mut self,
        _req: RequestVoteRequest,
    ) -> RequestVoteResponse {
        
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

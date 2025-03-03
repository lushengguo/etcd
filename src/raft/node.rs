use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use log::{error, info, warn};
use rand::Rng;
use serde::{Deserialize, Serialize};
use jsonrpc_core::{Error as RpcError, Result as RpcResult};
use jsonrpc_derive::rpc;
use jsonrpc_core_client::{RpcChannel, TypedClient};
use jsonrpc_core_client::transports::http;

// Raft状态
#[derive(Debug, Clone, PartialEq)]
pub enum NodeState {
    Follower,
    Candidate,
    Leader,
}

// 日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub term: u64,
    pub command: String,
    pub key: String,
    pub value: Option<String>,
}

// 远程节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteNode {
    pub node_uid: u64,
    pub address: String,
}

// AppendEntries请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntriesRequest {
    pub term: u64,
    pub leader_id: u64,
    pub prev_log_index: u64,
    pub prev_log_term: u64,
    pub entries: Vec<LogEntry>,
    pub leader_commit: u64,
}

// AppendEntries响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntriesResponse {
    pub term: u64,
    pub success: bool,
}

// RequestVote请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestVoteRequest {
    pub term: u64,
    pub candidate_id: u64,
    pub last_log_index: u64,
    pub last_log_term: u64,
}

// RequestVote响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestVoteResponse {
    pub term: u64,
    pub vote_granted: bool,
}

// Raft RPC接口
#[rpc]
pub trait RaftRpc {
    #[rpc(name = "appendEntries")]
    fn append_entries(&self, req: AppendEntriesRequest) -> RpcResult<AppendEntriesResponse>;
    
    #[rpc(name = "requestVote")]
    fn request_vote(&self, req: RequestVoteRequest) -> RpcResult<RequestVoteResponse>;
}

// 获取当前时间戳（毫秒）
fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as u64
}

// 本地节点
pub struct LocalNode {
    data: HashMap<String, String>, // 状态机
    client_to_cluster: HashMap<u64, TypedClient<RaftRpc>>,
    last_heartbeat: u64,
    election_timeout: u64,

    node_uid: u64,
    state: NodeState,

    // 持久状态
    term: u64,
    voted_for: Option<u64>,
    log: Vec<LogEntry>,

    // 易失状态
    commit_index: u64,
    last_applied: u64,

    // leader易失状态
    next_index: HashMap<u64, u64>,
    match_index: HashMap<u64, u64>,
}

impl LocalNode {
    pub fn new(id: u64, other_nodes: Vec<RemoteNode>) -> Self {
        let mut node = Self {
            data: HashMap::new(),
            client_to_cluster: HashMap::new(),
            last_heartbeat: current_time_millis(),
            election_timeout: Self::random_election_timeout(),
            node_uid: id,
            state: NodeState::Follower,
            term: 0,
            voted_for: None,
            log: vec![LogEntry {
                term: 0,
                command: "noop".to_string(),
                key: "".to_string(),
                value: None,
            }],
            commit_index: 0,
            last_applied: 0,
            next_index: HashMap::new(),
            match_index: HashMap::new(),
        };

        // 设置其他节点的初始next_index和match_index
        for remote in &other_nodes {
            node.next_index.insert(remote.node_uid, 1);
            node.match_index.insert(remote.node_uid, 0);
        }

        node
    }

    // 连接到集群
    pub async fn connect_to_cluster(&mut self, other_nodes: Vec<RemoteNode>) -> Result<(), Box<dyn std::error::Error>> {
        for node in &other_nodes {
            let uri = format!("http://{}", node.address);
            match http::connect::<RaftRpc>(&uri).await {
                Ok(client) => {
                self.client_to_cluster.insert(node.node_uid, client);
                    info!("Connected to node {}", node.node_uid);
                },
                Err(e) => {
                    error!("Failed to connect to node {}: {}", node.node_uid, e);
                }
            }
        }
        Ok(())
    }

    // 键值操作
    pub fn set(&mut self, key: String, value: String) -> jsonrpc_core::Error {
        if self.state != NodeState::Leader {
            return jsonrpc_core::Error::invalid_request();
        }
        
        let entry = LogEntry {
            term: self.term,
            command: "set".to_string(),
            key: key.clone(),
            value: Some(value.clone()),
        };
        
        self.log.push(entry);
        let _log_index = self.log.len() as u64 - 1;
        
        jsonrpc_core::Error::parse_error()
    }

    pub fn get(&mut self, key: String) -> Result<String, jsonrpc_core::Error> {
        match self.data.get(&key) {
            Some(value) => Ok(value.clone()),
            None => Err(jsonrpc_core::Error::invalid_params("Key not found")),
        }
    }

    pub fn del(&mut self, key: String) -> jsonrpc_core::Error {
        if self.state != NodeState::Leader {
            return jsonrpc_core::Error::invalid_request();
        }
        
        let entry = LogEntry {
            term: self.term,
            command: "del".to_string(),
            key: key.clone(),
            value: None,
        };
        
        self.log.push(entry);
        let _log_index = self.log.len() as u64 - 1;
        
        jsonrpc_core::Error::parse_error()
    }

    // 应用日志条目到状态机
    fn apply_log_entries(&mut self) {
        while self.last_applied < self.commit_index {
            self.last_applied += 1;
            let entry = &self.log[self.last_applied as usize];
            
            match entry.command.as_str() {
                "set" => {
                    if let Some(value) = &entry.value {
                        self.data.insert(entry.key.clone(), value.clone());
                    }
                },
                "del" => {
                    self.data.remove(&entry.key);
                },
                _ => {}
            }
        }
    }
    
    // 处理AppendEntries RPC
    pub fn append_entries_impl(
        &mut self,
        req: AppendEntriesRequest,
    ) -> RpcResult<AppendEntriesResponse> {
        // 实现AppendEntries逻辑
        // ...简化实现...
        
        Ok(AppendEntriesResponse {
            term: self.term,
            success: true,
        })
    }

    // 处理RequestVote RPC
    pub fn request_vote_impl(
        &mut self,
        req: RequestVoteRequest,
    ) -> RpcResult<RequestVoteResponse> {
        // 实现RequestVote逻辑
        // ...简化实现...
        
        Ok(RequestVoteResponse {
            term: self.term,
            vote_granted: true,
        })
    }
    
    // 随机生成选举超时时间
    fn random_election_timeout() -> u64 {
        let mut rng = rand::thread_rng();
        rng.gen_range(150..300)
    }
}

// RaftRpc实现
pub struct RaftRpcImpl {
    node: Arc<tokio::sync::Mutex<LocalNode>>,
}

impl RaftRpcImpl {
    pub fn new(node: Arc<tokio::sync::Mutex<LocalNode>>) -> Self {
        Self { node }
    }
}

#[jsonrpc_derive::rpc]
impl RaftRpc for RaftRpcImpl {
    fn append_entries(&self, req: AppendEntriesRequest) -> RpcResult<AppendEntriesResponse> {
        // 这里需要一个tokio运行时来处理异步锁
        // 简化起见，直接返回成功
        Ok(AppendEntriesResponse {
            term: 0,
            success: true,
        })
    }
    
    fn request_vote(&self, req: RequestVoteRequest) -> RpcResult<RequestVoteResponse> {
        // 同样简化处理
        Ok(RequestVoteResponse {
            term: 0,
            vote_granted: true,
        })
    }
}

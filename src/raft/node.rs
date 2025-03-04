use std::collections::HashMap;
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use log::info;
use std::sync::Arc;
use tokio::sync::Mutex;
use jsonrpc_core::Result as RpcResult;
use jsonrpc_core_client::TypedClient;
use jsonrpc_core_client::transports::http;

// 导入 rpc 模块中的类型
use crate::raft::rpc::{
    RaftRpc, AppendEntriesRequest, AppendEntriesResponse,
    RequestVoteRequest, RequestVoteResponse, LogEntry
};

// 节点状态枚举
#[derive(Debug, Clone, PartialEq)]
pub enum NodeState {
    Follower,
    Candidate,
    Leader,
}

// 远程节点信息
#[derive(Debug, Clone)]
pub struct RemoteNode {
    pub node_uid: u64,
    pub addr: String,
}

// 本地节点实现
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
    // 使用 String 作为客户端类型，因为我们不能直接使用 RaftRpc trait
    pub client_to_cluster: HashMap<u64, String>, // 存储节点地址而不是客户端
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

    // 连接到集群中的其他节点
    pub async fn connect_to_cluster(&mut self, nodes: Vec<RemoteNode>) -> Result<(), Box<dyn Error>> {
        for node in nodes {
            // 只存储节点地址，不尝试建立连接
            self.client_to_cluster.insert(node.node_uid, node.addr.clone());
            info!("Added node {} at {}", node.node_uid, node.addr);
        }
        Ok(())
    }

    // 设置键值对
    pub async fn set(&mut self, key: String, value: String) -> RpcResult<()> {
        // 只有 Leader 可以处理写请求
        if self.state != NodeState::Leader {
            return Err(jsonrpc_core::Error::invalid_request());
        }

        // 创建日志条目
        let log_entry = LogEntry {
            term: self.current_term,
            index: self.log.len() as u64 + 1,
            command: format!("SET {} {}", key, value),
        };

        // 添加到本地日志
        self.log.push(log_entry.clone());

        // 复制到其他节点
        self.replicate_log().await;

        Ok(())
    }

    // 获取键值
    pub async fn get(&self, key: String) -> RpcResult<String> {
        // 简化实现，直接返回空字符串
        Ok(String::new())
    }

    // 复制日志到其他节点
    async fn replicate_log(&self) -> bool {
        // 简化实现，假设复制成功
        true
    }

    // 处理附加日志 RPC
    pub async fn handle_append_entries(
        &mut self,
        _req: AppendEntriesRequest,
    ) -> AppendEntriesResponse {
        // 简化实现
        AppendEntriesResponse {
            term: self.current_term,
            success: true,
        }
    }

    // 处理请求投票 RPC
    pub async fn handle_request_vote(
        &mut self,
        _req: RequestVoteRequest,
    ) -> RequestVoteResponse {
        // 简化实现
        RequestVoteResponse {
            term: self.current_term,
            vote_granted: false,
        }
    }
}

// RPC 实现
pub struct RaftRpcImpl {
    node: Arc<Mutex<LocalNode>>,
}

impl RaftRpcImpl {
    pub fn new(node: Arc<Mutex<LocalNode>>) -> Self {
        Self { node }
    }
}

// 实现 RaftRpc trait
impl RaftRpc for RaftRpcImpl {
    fn append_entries(&self, _req: AppendEntriesRequest) -> RpcResult<AppendEntriesResponse> {
        // 获取运行时处理异步操作
        let rt = tokio::runtime::Runtime::new().unwrap();
        let node = self.node.clone();
        
        // 在运行时中执行异步操作
        let response = rt.block_on(async move {
            let mut node = node.lock().await;
            node.handle_append_entries(_req).await
        });
        
        Ok(response)
    }

    fn request_vote(&self, _req: RequestVoteRequest) -> RpcResult<RequestVoteResponse> {
        // 获取运行时处理异步操作
        let rt = tokio::runtime::Runtime::new().unwrap();
        let node = self.node.clone();
        
        // 在运行时中执行异步操作
        let response = rt.block_on(async move {
            let mut node = node.lock().await;
            node.handle_request_vote(_req).await
        });
        
        Ok(response)
    }
}

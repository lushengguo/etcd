use jsonrpc_http_server::ServerBuilder;
use jsonrpc_core::IoHandler;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use serde_json::from_reader;
use std::fs::File;

use etcd::rpc::{EtcdRpc, EtcdRpcImpl};
use etcd::raft::node::{LocalNode, RemoteNode, RaftRpc, RaftRpcImpl};
use log::info;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    env_logger::init();
    
    // 获取命令行参数
    let args: Vec<String> = std::env::args().collect();
    let addr = if args.len() > 1 {
        args[1].parse()?
    } else {
        "127.0.0.1:2379".parse()?
    };
    
    let raft_config_path = if args.len() > 2 {
        &args[2]
    } else {
        "raft_configuration.json"
    };
    
    let raft_uid = if args.len() > 3 {
        args[3].parse()?
    } else {
        1
    };
    
    // 读取Raft配置
    let config_file = File::open(raft_config_path)?;
    let remote_config: Vec<RemoteNode> = from_reader(config_file)?;
    
    // 创建Raft节点
    let raft_node = Arc::new(Mutex::new(LocalNode::new(raft_uid, remote_config.clone())));
    
    info!("启动服务器在地址 {}", addr);
    
    // 注册EtcdRPC服务
    let mut io = IoHandler::default();
    let etcd_rpc = EtcdRpcImpl::new();
    io.extend_with(etcd_rpc.to_delegate());
    
    // 注册RaftRPC服务
    let raft_rpc = RaftRpcImpl::new(raft_node);
    io.extend_with(raft_rpc.to_delegate());

    // 启动服务器
    let server = ServerBuilder::new(io)
        .start_http(&addr)
        .expect("服务器启动失败");

    info!("服务器已启动");
    server.wait();
    
    Ok(())
}

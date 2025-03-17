use env_logger::Builder;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time;
use tonic::transport::Server;

use etcd::etcd_rpc::EtcdRpcImpl;
use etcd::raft::node::{LocalNode, RemoteNode};
use etcd::raft::raft_service::RaftRpcService;
use log::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Builder::from_env("RUST_LOG")
        .format(|buf, record| {
            writeln!(
                buf,
                "[{} {}:{}] - {}",
                record.level(),
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.args()
            )
        })
        .init();

    let args: Vec<String> = std::env::args().collect();
    
    // 默认值
    let mut etcd_addr = "127.0.0.1:2379".to_string();
    let mut raft_addr = "127.0.0.1:2380".to_string();
    let mut node_id: u64 = 1;
    let mut cluster_conf = String::new();
    
    // 解析命名参数
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--node-id" => {
                if i + 1 < args.len() {
                    node_id = args[i + 1].parse()?;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--etcd-port" => {
                if i + 1 < args.len() {
                    etcd_addr = format!("127.0.0.1:{}", args[i + 1]);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--raft-port" => {
                if i + 1 < args.len() {
                    raft_addr = format!("127.0.0.1:{}", args[i + 1]);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--cluster-conf" => {
                if i + 1 < args.len() {
                    cluster_conf = args[i + 1].clone();
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }
    
    // 如果没有指定集群配置，则使用默认值
    if cluster_conf.is_empty() {
        cluster_conf = format!("{}={}", node_id, raft_addr);
    }

    let etcd_addr: SocketAddr = etcd_addr.parse()?;
    let raft_addr: SocketAddr = raft_addr.parse()?;

    info!("Starting node ID: {}", node_id);
    info!("Starting etcd service at address {}", etcd_addr);
    info!("Starting raft service at address {}", raft_addr);
    info!("Cluster configuration: {}", cluster_conf);

    let node = Arc::new(Mutex::new(LocalNode::new(node_id)));

    {
        let mut node_guard = node.lock().await;
        let mut cluster_nodes = Vec::new();

        for node_conf in cluster_conf.split(',') {
            let parts: Vec<&str> = node_conf.split('=').collect();
            if parts.len() == 2 {
                let id: u64 = parts[0].parse()?;
                let addr = parts[1].to_string();

                cluster_nodes.push(RemoteNode { node_uid: id, addr });
            }
        }

        node_guard.connect_to_cluster(cluster_nodes).await?;
    }

    let node_clone = node.clone();
    let heartbeat_node = node.clone();

    let etcd_service = EtcdRpcImpl::new(node);
    let etcd_server = Server::builder()
        .add_service(etcd_service.server())
        .serve(etcd_addr);

    let raft_service = RaftRpcService::new(node_clone);
    let raft_server = Server::builder()
        .add_service(raft_service.server())
        .serve(raft_addr);

    let heartbeat_task = tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(10));

        loop {
            interval.tick().await;

            let mut node_guard = heartbeat_node.lock().await;
            node_guard.handle_heartbeat_timeout().await;
        }
    });

    tokio::select! {
        _ = etcd_server => {
            info!("etcd service has stopped");
        }
        _ = raft_server => {
            info!("raft service has stopped");
        }
        _ = heartbeat_task => {
            info!("heartbeat check task has stopped");
        }
    }

    Ok(())
}

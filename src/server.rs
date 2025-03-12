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

    let etcd_addr: SocketAddr = if args.len() > 1 {
        args[1].parse()?
    } else {
        "127.0.0.1:2379".parse()?
    };

    let raft_addr: SocketAddr = if args.len() > 2 {
        args[2].parse()?
    } else {
        "127.0.0.1:2380".parse()?
    };

    let node_id: u64 = if args.len() > 3 { args[3].parse()? } else { 1 };

    let cluster_conf = if args.len() > 4 {
        args[4].clone()
    } else {
        format!("{}={}", node_id, raft_addr)
    };

    info!("启动节点 ID: {}", node_id);
    info!("启动 etcd 服务在地址 {}", etcd_addr);
    info!("启动 raft 服务在地址 {}", raft_addr);
    info!("集群配置: {}", cluster_conf);

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
            info!("etcd 服务已停止");
        }
        _ = raft_server => {
            info!("raft 服务已停止");
        }
        _ = heartbeat_task => {
            info!("心跳检查任务已停止");
        }
    }

    Ok(())
}

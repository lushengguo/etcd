use env_logger::Builder;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Server;

use etcd::etcd_rpc::EtcdRpcImpl;
use etcd::raft::node::LocalNode;
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
    let addr: SocketAddr = if args.len() > 1 {
        args[1].parse()?
    } else {
        "127.0.0.1:2379".parse()?
    };

    info!("启动服务器在地址 {}", addr);

    // 创建节点
    let node = Arc::new(Mutex::new(LocalNode::new(1)));

    // 创建 gRPC 服务
    let etcd_service = EtcdRpcImpl::new(node);

    // 启动 gRPC 服务器
    Server::builder()
        .add_service(etcd_service.server())
        .serve(addr)
        .await?;

    Ok(())
}

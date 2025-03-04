use jsonrpc_http_server::{ServerBuilder, DomainsValidation};
use jsonrpc_core::IoHandler;
use std::net::SocketAddr;

// 导入 rpc 模块
use etcd::etcd_rpc::{EtcdRpc, EtcdRpcImpl};
use log::info;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    env_logger::init();
    
    // 获取地址参数
    let args: Vec<String> = std::env::args().collect();
    let addr: SocketAddr = if args.len() > 1 {
        args[1].parse()?
    } else {
        "127.0.0.1:2379".parse()?
    };
    
    info!("启动服务器在地址 {}", addr);
    
    let mut io = IoHandler::default();
    let rpc = EtcdRpcImpl::new();
    io.extend_with(rpc.to_delegate());

    let server = ServerBuilder::new(io)
        .cors(DomainsValidation::Disabled)
        .start_http(&addr)
        .expect("服务器启动失败");

    info!("服务器已启动");
    server.wait();
    
    Ok(())
}

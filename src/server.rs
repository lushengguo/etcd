use etcd_protobufs::etcd_server::{Etcd, EtcdServer};
use etcd_protobufs::{EtcdRequest, EtcdResponse};
use tonic::{transport::Server, Request, Response, Status};

use std::collections::HashMap;
use std::env;
use std::sync::Mutex;

use etcd::raft::LocalNode;
use etcd::raft::RemoteNode;

pub mod etcd_protobufs {
    tonic::include_proto!("etcd_protobufs");
}

#[derive(Debug)]
pub struct EtcdRpcServer {
    node: Mutex<LocalNode>,
}

impl EtcdRpcServer {
    pub fn new(id: u64, remote: Vec<RemoteNode>) -> Self {
        EtcdRpcServer {
            node: Mutex::new(LocalNode::new(id, remote)),
        }
    }
}

#[tonic::async_trait]
impl Etcd for EtcdRpcServer {
    async fn set(&self, request: Request<EtcdRequest>) -> Result<Response<EtcdResponse>, Status> {
        let req = request.into_inner();
        let mut node = self
            .node
            .lock()
            .map_err(|e| Status::internal(format!("Mutex lock error: {}", e)))?;
        let status = node.set(req.key.clone(), req.value.clone());
        if status.code() != tonic::Code::Ok {
            return Err(status);
        }
        let reply = EtcdResponse {
            ok: true,
            ..Default::default()
        };
        Ok(Response::new(reply))
    }

    async fn get(&self, request: Request<EtcdRequest>) -> Result<Response<EtcdResponse>, Status> {
        let req = request.into_inner();
        let mut node = self
            .node
            .lock()
            .map_err(|e| Status::internal(format!("Mutex lock error: {}", e)))?;
        let value = node.get(req.key.clone())?;
        let reply = EtcdResponse {
            ok: true,
            value: value,
            ..Default::default()
        };
        Ok(Response::new(reply))
    }

    async fn del(&self, request: Request<EtcdRequest>) -> Result<Response<EtcdResponse>, Status> {
        let req = request.into_inner();
        let mut node = self
            .node
            .lock()
            .map_err(|e| Status::internal(format!("Mutex lock error: {}", e)))?;
        let status = node.del(req.key.clone());
        if status.code() != tonic::Code::Ok {
            return Err(status);
        }
        let reply = EtcdResponse {
            ok: true,
            ..Default::default()
        };
        Ok(Response::new(reply))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "Usage: {} <config_file_path> <current_process_raft_id>",
            args[0]
        );
        std::process::exit(1);
    }

    let json_config = std::fs::read_to_string(&args[1])?;
    let remote_config: Vec<RemoteNode> = serde_json::from_str(&json_config)?;
    let raft_id = args[2].parse()?;

    let addr = "127.0.0.1:50051".parse()?;
    let etcd = EtcdRpcServer::new(raft_id, remote_config);
    Server::builder()
        .add_service(EtcdServer::new(etcd))
        .serve(addr)
        .await?;

    Ok(())
}

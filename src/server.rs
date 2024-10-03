use etcd_proto::etcd_server::{Etcd, EtcdServer};
use etcd_proto::{Request as EtcdRequest, Response as EtcdResponse};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::Mutex;
use tonic::{transport::Server, Request, Response, Status};

pub mod etcd_proto {
    tonic::include_proto!("etcd_proto");
}

lazy_static! {
    static ref KV_STORE: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
}

#[derive(Debug, Default)]
pub struct EtcdRpcServer {}

#[tonic::async_trait]
impl Etcd for EtcdRpcServer {
    async fn set(&self, request: Request<EtcdRequest>) -> Result<Response<EtcdResponse>, Status> {
        let req = request.into_inner();
        let mut store = KV_STORE.lock().unwrap();
        store.insert(req.key.clone(), req.value.clone());
        let reply = EtcdResponse {
            ok: true,
            key: req.key,
            value: req.value,
        };
        Ok(Response::new(reply))
    }

    async fn get(&self, request: Request<EtcdRequest>) -> Result<Response<EtcdResponse>, Status> {
        let req = request.into_inner();
        let store = KV_STORE.lock().unwrap();
        let value = store.get(&req.key).cloned().unwrap_or_default();
        let reply = EtcdResponse {
            ok: true,
            key: req.key,
            value,
        };
        Ok(Response::new(reply))
    }

    async fn del(&self, request: Request<EtcdRequest>) -> Result<Response<EtcdResponse>, Status> {
        let req = request.into_inner();
        let mut store = KV_STORE.lock().unwrap();
        let value = store.remove(&req.key).unwrap_or_default();
        let reply = EtcdResponse {
            ok: true,
            key: req.key,
            value,
        };
        Ok(Response::new(reply))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:50051".parse().unwrap();
    let etcd = EtcdRpcServer::default();

    Server::builder()
        .add_service(EtcdServer::new(etcd))
        .serve(addr)
        .await?;

    Ok(())
}
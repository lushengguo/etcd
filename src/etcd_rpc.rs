use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};

use crate::proto::etcd_service_server::{EtcdService, EtcdServiceServer};
use crate::proto::{
    DeleteRequest, DeleteResponse, GetRequest, GetResponse, SetRequest, SetResponse,
};
use crate::raft::node::LocalNode;

#[derive(Clone, Debug)]
pub struct KeyValue {
    pub key: String,
    pub value: String,
}

pub struct EtcdRpcImpl {
    node: Arc<Mutex<LocalNode>>,
}

impl EtcdRpcImpl {
    pub fn new(node: Arc<Mutex<LocalNode>>) -> Self {
        Self { node }
    }

    pub fn server(self) -> EtcdServiceServer<Self> {
        EtcdServiceServer::new(self)
    }
}

#[tonic::async_trait]
impl EtcdService for EtcdRpcImpl {
    async fn set(&self, request: Request<SetRequest>) -> Result<Response<SetResponse>, Status> {
        let req = request.into_inner();
        let key = req.key;
        let value = req.value;

        let mut node_guard = self.node.lock().await;
        match node_guard.set(key, value).await {
            Ok(_) => Ok(Response::new(SetResponse { success: true })),
            Err(e) => Err(Status::internal(format!("Internal error: {:?}", e))),
        }
    }

    async fn get(&self, request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let req = request.into_inner();
        let key = req.key;

        let node_guard = self.node.lock().await;
        match node_guard.get(key).await {
            Ok(value) => Ok(Response::new(GetResponse { value })),
            Err(_) => Err(Status::not_found("Key does not exist")),
        }
    }

    async fn delete(
        &self,
        request: Request<DeleteRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let req = request.into_inner();
        let key = req.key;

        let mut node_guard = self.node.lock().await;
        match node_guard.delete(key).await {
            Ok(_) => Ok(Response::new(DeleteResponse { success: true })),
            Err(e) => Err(Status::internal(format!("Internal error: {:?}", e))),
        }
    }
}

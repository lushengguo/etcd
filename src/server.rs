use etcd::etcd_protobufs::etcd_server::{Etcd, EtcdServer};
use etcd::etcd_protobufs::{EtcdRequest, EtcdResponse};
use etcd::raft::node::{LocalNode, RemoteNode};
use etcd::raft_protobufs::raft_server::{Raft, RaftServer};
use etcd::raft_protobufs::{
    AppendEntriesRequest, AppendEntriesResponse, RequestVoteRequest, RequestVoteResponse,
};
use tonic::{transport::Server, Request, Response, Status};

use chrono::Utc;
use env_logger::{Builder, Env};
use log::{error, info, warn};
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use tokio::sync::Mutex;

struct LoggerConfig {
    node_id: u64,
}

fn init_logger(node_id: u64) {
    let env = Env::default().filter_or("RUST_LOG", "debug");
    let log_file_path = format!("./log/node_{}.log", node_id);

    // 创建日志目录
    std::fs::create_dir_all("./log").expect("Unable to create log directory");

    let log_file = File::create(log_file_path).expect("Unable to create log file");
    let log_file = Arc::new(Mutex::new(log_file));

    Builder::from_env(env)
        .format(move |buf, record| {
            let thread_id = format!("{:?}", thread::current().id());
            let thread_id = thread_id
                .trim_start_matches("ThreadId(")
                .trim_end_matches(')');
            let file = record.file().unwrap_or("unknown");
            let file_name = Path::new(file)
                .file_name()
                .unwrap_or_default()
                .to_str()
                .unwrap_or("unknown");
            let line = record.line().unwrap_or(0);
            let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string();
            let module_path = record.module_path().unwrap_or("unknown");
            let log_message = format!(
                "{} node[{}] {} [{}:{}:{}] [tid={}] - {}",
                timestamp,
                node_id,
                record.level(),
                file_name,
                line,
                module_path,
                thread_id,
                record.args()
            );

            writeln!(buf, "{}", log_message).unwrap();

            let log_file = Arc::clone(&log_file);
            tokio::spawn(async move {
                let mut log_file = log_file.lock().await;
                writeln!(log_file, "{}", log_message).unwrap();
            });

            Ok(())
        })
        .init();
}

#[derive(Debug)]
pub struct EtcdRpcServer {
    node: Arc<Mutex<LocalNode>>,
}

impl EtcdRpcServer {
    pub fn new(id: u64, remote: Vec<RemoteNode>) -> Self {
        EtcdRpcServer {
            node: Arc::new(Mutex::new(LocalNode::new(id, remote))),
        }
    }
}

#[tonic::async_trait]
impl Etcd for EtcdRpcServer {
    async fn set(&self, request: Request<EtcdRequest>) -> Result<Response<EtcdResponse>, Status> {
        let req = request.into_inner();
        let mut node = self.node.lock().await;
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
        let mut node = self.node.lock().await;
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
        let mut node = self.node.lock().await;
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

pub struct LocalNodeWrapper(pub Arc<Mutex<LocalNode>>);

#[tonic::async_trait]
impl Raft for LocalNodeWrapper {
    async fn append_entries(
        &self,
        request: Request<AppendEntriesRequest>,
    ) -> Result<Response<AppendEntriesResponse>, Status> {
        let mut raft_node = self.0.lock().await;
        raft_node.append_entries_impl(request)
    }

    async fn request_vote(
        &self,
        request: Request<RequestVoteRequest>,
    ) -> Result<Response<RequestVoteResponse>, Status> {
        let mut raft_node = self.0.lock().await;
        raft_node.request_vote_impl(request)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!(
            "Usage: {} <etcd_server_listen_address> <config_file_path> <current_process_raft_uid>",
            args[0]
        );
        std::process::exit(1);
    }

    let etcd_listen_address = args[1].parse()?;
    let json_config = std::fs::read_to_string(&args[2])?;
    let remote_config: Vec<RemoteNode> = serde_json::from_str(&json_config)?;
    let raft_uid = args[3].parse()?;
    let raft_listen_address = remote_config
        .iter()
        .find(|node| node.node_uid == raft_uid)
        .expect("Current node not found in config file")
        .address
        .parse()?;

    init_logger(raft_uid);
    info!(
        "reading configuration from stdin or command line ok, etcd run on {}, raft run on {}",
        etcd_listen_address, raft_listen_address
    );

    let raft_node = Arc::new(Mutex::new(LocalNode::new(raft_uid, remote_config)));
    let raft_wrapper = LocalNodeWrapper(Arc::clone(&raft_node));
    let etcd = EtcdRpcServer {
        node: Arc::clone(&raft_node),
    };

    let etcd_server = Server::builder()
        .add_service(EtcdServer::new(etcd))
        .serve(etcd_listen_address);

    let raft_server = Server::builder()
        .add_service(RaftServer::new(raft_wrapper))
        .serve(raft_listen_address);

    tokio::spawn(async move {
        let mut mutable_node = raft_node.lock().await;
        mutable_node.periodic_check_election_timeout().await;
    });

    tokio::try_join!(etcd_server, raft_server)?;

    Ok(())
}

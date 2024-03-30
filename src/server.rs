use std::collections::HashMap;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

enum RaftState {
    Follower,
    Candidate,
    Leader,
}

type Term = u64;
type LogIndex = u64;
type NodeId = u64;
type DistributedConsistentData = HashMap<String, String>;

pub fn process_name() -> String {
    format!("node {}", std::env::args().nth(1).unwrap())
}

async fn new_connection_handler(_socket: TcpStream) {}

async fn serve(port: u32) {
    let address = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(address)
        .await
        .expect("Failed to bind to address");

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        tokio::spawn(new_connection_handler(socket));
    }
}

pub async fn start_server() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <process_index: u32>", args[0]);
        std::process::exit(1);
    } else {
        let port = 5000 + std::env::args().nth(1).unwrap().parse::<u32>().unwrap();

        serve(port).await;
    }
}

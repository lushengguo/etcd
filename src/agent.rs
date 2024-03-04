use crate::operation;
use operation::Operation;
use tokio::io::{self, AsyncReadExt};
use tokio::net::TcpListener;
use tokio::net::TcpStream;

// async fn get_leader_node_connection() -> &TcpStream {}

async fn redirect_to_leader_node(operation: String) {}

async fn new_connection_handler(mut socket: TcpStream) {
    let mut payload: u64 = 0;
    let mut payload_buf = vec![0u8; 8];

    loop {
        let n = socket
            .read_exact(&mut payload_buf)
            .await
            .expect("failed to read data from socket");

        if n == 0 || n != 8 {
            return;
        }

        payload = u64::from_le_bytes([
            payload_buf[0],
            payload_buf[1],
            payload_buf[2],
            payload_buf[3],
            payload_buf[4],
            payload_buf[5],
            payload_buf[6],
            payload_buf[7],
        ]);

        if payload == 0 || payload > 1024 {
            println!("invalid payload size: {}", payload);
            return;
        }

        let mut buf = vec![0u8; payload as usize];

        let n = socket
            .read_exact(&mut buf)
            .await
            .expect("failed to read data from socket");

        if n == 0 || n != payload as usize {
            return;
        }

        match Operation::new(&buf) {
            Ok(operation) => {
                let data = String::from_utf8(buf.to_vec()).unwrap();
                redirect_to_leader_node(data).await;
            }
            Err(err) => println!("parse operation failed: {}", err),
        };
    }
}

pub async fn start_agent() {
    let address = format!("127.0.0.1:5000");
    let listener = TcpListener::bind(address)
        .await
        .expect("Failed to bind to address");

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        tokio::spawn(new_connection_handler(socket));
    }
}

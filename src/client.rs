use std::collections::HashMap;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use regex::Regex;
use tokio::io::AsyncWriteExt;

// mod net;

async fn connect(address: &String) -> Result<TcpStream, tokio::io::Error> {
    match TcpStream::connect(address).await {
        Ok(socket) => {
            println!("Connected to server!");
            Ok(socket)
        }
        Err(e) => {
            eprintln!("Failed to connect to server: {}", e);
            Err(e)
        }
    }
}

pub async fn start_client() {
    let address = "127.0.0.1:5000".to_string();
    match connect(&address).await // agent 
    {
        Ok(mut socket) => {
            let mut input = String::new();
            let re = Regex::new(r"(Insert|Modify|Delete) (\w+) (\w+)").unwrap();
            match std::io::stdin().read_line(&mut input) {
                Ok(_) => {
                    if let Some(captures) = re.captures(input.as_str()) {
                        let payload = input.len() as u64;
                        let mut payload_buf = [0u8; 8];
                        payload_buf.copy_from_slice(&payload.to_be_bytes());
                        socket.write_all(&payload_buf).await.expect("failed to write data to socket");

                        let mut data_buf = vec![0u8; payload as usize];
                        data_buf.copy_from_slice(input.as_bytes());
                        socket.write_all(&data_buf).await.expect("failed to write data to socket");

                    } else {
                        println!("input is not valid, pattern should be operation key value ,and operation should be Insert, Modify or Delete.");
                    }
                }
                Err(error) => {
                    eprintln!("Error reading from stdin: {}", error);
                    return;
                }
            }
        },
        Err(error)=>{
            eprintln!("connect agent server failed: {}", error);
            return;
        }        
    }

}

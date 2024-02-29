use std::collections::HashMap;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use regex::Regex;

mod net;

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
    match connect("127.0.0.1:5000").await // agent 
    {
        Ok(socket) => {
            let mut input = String::new();
            let re = Regex::new(r"op=(Insert|Modify|Delete),key=(\w+),value=(\w+)").unwrap();
            match std::io::stdin().read_line(&mut input) {
                Ok(_) => {
                    let mut serialized_op = String::new();

                    if let Some(captures) = re.captures(text) {
                        if let Some(op) = captures.get(1) {
                            serialized_op.push(op);
                        }

                        if let Some(key) = captures.get(2) {
                            serialized_op.push(key);
                        }

                        if let Some(value) = captures.get(3) {
                            serialized_op.push(value);
                        }
                    } else {
                        println!("No match found.");
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

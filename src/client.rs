use env_logger::Builder;
use jsonrpc_core_client::transports::http;
use log::info;
use std::env;
use std::error::Error;
use std::io::Write;

use etcd::etcd_rpc::{EtcdRpc, KeyValue};

#[jsonrpc_derive::rpc(client)]
pub trait ClientEtcdRpc {
    #[rpc(name = "set", returns = "KeyValue")]
    fn set(&self, key: String, value: String) -> jsonrpc_core::Result<KeyValue>;

    #[rpc(name = "get", returns = "KeyValue")]
    fn get(&self, key: String) -> jsonrpc_core::Result<KeyValue>;

    #[rpc(name = "del", returns = "KeyValue")]
    fn del(&self, key: String) -> jsonrpc_core::Result<KeyValue>;
}

pub struct Client {
    client: jsonrpc_core_client::RpcChannel,
}

impl Client {
    pub async fn connect(addr: &str) -> Result<Self, Box<dyn Error>> {
        let url = format!("http://{}", addr);
        let client = http::connect(&url).await?;
        Ok(Self { client })
    }

    pub async fn set(&self, key: &str, value: &str) -> Result<KeyValue, Box<dyn Error>> {
        let client = ClientEtcdRpcClient::new(self.client.clone());
        let response = client.set(key.to_string(), value.to_string()).await?;
        Ok(response)
    }

    pub async fn get(&self, key: &str) -> Result<KeyValue, Box<dyn Error>> {
        let client = ClientEtcdRpcClient::new(self.client.clone());
        let response = client.get(key.to_string()).await?;
        Ok(response)
    }

    pub async fn delete(&self, key: &str) -> Result<KeyValue, Box<dyn Error>> {
        let client = ClientEtcdRpcClient::new(self.client.clone());
        let response = client.del(key.to_string()).await?;
        Ok(response)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
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

    let client = Client::connect("127.0.0.1:2379").await?;

    let response = client.set("test_key", "test_value").await?;
    info!("Set response: {:?}", response);

    let response = client.get("test_key").await?;
    info!("Get response: {:?}", response);

    let response = client.delete("test_key").await?;
    info!("Delete response: {:?}", response);

    Ok(())
}

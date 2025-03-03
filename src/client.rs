use jsonrpc_core_client::{RpcChannel, TypedClient};
use jsonrpc_core_client::transports::http;
use std::error::Error;

use etcd::rpc::{EtcdRpc, KeyValue};

pub struct Client {
    client: TypedClient<EtcdRpc>,
}

impl Client {
    pub async fn connect(addr: &str) -> Result<Self, Box<dyn Error>> {
        let uri = format!("http://{}", addr);
        let client = http::connect::<EtcdRpc>(&uri).await?;
        Ok(Self { client })
    }

    pub async fn set(&self, key: &str, value: &str) -> Result<KeyValue, Box<dyn Error>> {
        let response = self.client.set(key.to_string(), value.to_string()).await?;
        Ok(response)
    }

    pub async fn get(&self, key: &str) -> Result<KeyValue, Box<dyn Error>> {
        let response = self.client.get(key.to_string()).await?;
        Ok(response)
    }

    pub async fn delete(&self, key: &str) -> Result<KeyValue, Box<dyn Error>> {
        let response = self.client.del(key.to_string()).await?;
        Ok(response)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = Client::connect("127.0.0.1:2379").await?;

    // 设置键值对
    let response = client.set("test_key", "test_value").await?;
    println!("Set response: {:?}", response);

    // 获取值
    let response = client.get("test_key").await?;
    println!("Get response: {:?}", response);

    // 删除键值对
    let response = client.delete("test_key").await?;
    println!("Delete response: {:?}", response);

    Ok(())
}

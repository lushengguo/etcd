use jsonrpc_core_client::transports::http;
use serde::{Deserialize, Serialize};
use std::error::Error;

// 导入 rpc 模块中的 EtcdRpc trait
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
    let client = Client::connect("127.0.0.1:2379").await?;

    // 示例：设置键值对
    let response = client.set("test_key", "test_value").await?;
    println!("Set response: {:?}", response);

    // 示例：获取值
    let response = client.get("test_key").await?;
    println!("Get response: {:?}", response);

    // 示例：删除键值对
    let response = client.delete("test_key").await?;
    println!("Delete response: {:?}", response);

    Ok(())
}

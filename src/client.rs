use env_logger::Builder;
use log::info;
use std::error::Error;
use std::io::Write;
use tonic::transport::Channel;

use etcd::proto::{etcd_service_client::EtcdServiceClient, DeleteRequest, GetRequest, SetRequest};

pub struct Client {
    client: EtcdServiceClient<Channel>,
}

impl Client {
    pub async fn connect(addr: &str) -> Result<Self, Box<dyn Error>> {
        let url = format!("http://{}", addr);
        let client = EtcdServiceClient::connect(url).await?;
        Ok(Self { client })
    }

    pub async fn set(&mut self, key: &str, value: &str) -> Result<bool, Box<dyn Error>> {
        let request = SetRequest {
            key: key.to_string(),
            value: value.to_string(),
        };

        let response = self.client.set(request).await?;
        Ok(response.into_inner().success)
    }

    pub async fn get(&mut self, key: &str) -> Result<String, Box<dyn Error>> {
        let request = GetRequest {
            key: key.to_string(),
        };

        let response = self.client.get(request).await?;
        Ok(response.into_inner().value)
    }

    pub async fn delete(&mut self, key: &str) -> Result<bool, Box<dyn Error>> {
        let request = DeleteRequest {
            key: key.to_string(),
        };

        let response = self.client.delete(request).await?;
        Ok(response.into_inner().success)
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

    let mut client = Client::connect("127.0.0.1:2379").await?;

    let success = client.set("test_key", "test_value").await?;
    info!("Set key-value pair: {}", success);

    let value = client.get("test_key").await?;
    info!("Get value: {}", value);

    let success = client.delete("test_key").await?;
    info!("Delete key-value pair: {}", success);

    Ok(())
}

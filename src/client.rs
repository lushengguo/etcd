use etcd_proto::etcd_client::EtcdClient;
use etcd_proto::Request as EtcdRequest;

pub mod etcd_proto {
    tonic::include_proto!("etcd_proto");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = EtcdClient::connect("http://127.0.0.1:50051").await?;

    let request = tonic::Request::new(EtcdRequest {
        request_id: 1,
        key: "my_key".to_string(),
        value: "my_value".to_string(),
    });

    let response = client.set(request).await?;
    println!("RESPONSE={:?}", response);

    Ok(())
}
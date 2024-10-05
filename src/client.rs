use etcd::etcd_protobufs::etcd_client::EtcdClient;
use etcd::etcd_protobufs::EtcdRequest;

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

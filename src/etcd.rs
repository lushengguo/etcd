use etcd_rs::*;
use tokio::runtime::Runtime;

pub async fn register_service(
    client: &Client,
    service_name: &str,
    service_info: &str,
) -> Result<LeaseId, EtcdError> {
    // Create a lease for the service
    let lease = client.lease().grant(LeaseGrantRequest::new(10)).await?;
    let lease_id = lease.id();

    // Register the service with the lease
    let key = format!("/services/{}/info", service_name);
    let put_req = PutRequest::new(key, service_info).with_lease(lease_id);
    client.kv().put(put_req).await?;

    Ok(lease_id)
}

pub async fn keep_service_alive(client: &Client, lease_id: LeaseId) -> Result<(), EtcdError> {
    client
        .lease()
        .keep_alive(LeaseKeepAliveRequest::new(lease_id))
        .await?;
    Ok(())
}

pub async fn discover_services(client: &Client) -> Result<Vec<String>, EtcdError> {
    let key_prefix = "/services/";
    let resp = client
        .kv()
        .range(RangeRequest::new(KeyRange::prefix(key_prefix)))
        .await?;

    let mut services = Vec::new();
    for kv in resp.kvs() {
        let service_info = String::from_utf8_lossy(kv.value()).into_owned();
        services.push(service_info);
    }

    Ok(services)
}

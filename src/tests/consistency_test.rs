#[cfg(test)]
mod tests {
    use crate::client::Client;
    use std::time::Duration;
    use tokio::time::sleep;

    async fn setup_clients() -> (Client, Client) {
        let client1 = Client::connect("127.0.0.1:2379").await.unwrap();
        let client2 = Client::connect("127.0.0.1:2380").await.unwrap();
        (client1, client2)
    }

    #[tokio::test]
    async fn test_consistency_violation() {
        let (mut client1, mut client2) = setup_clients().await;
        
        
        
        let set_result = client1.set("key1", "value1").await.unwrap();
        assert!(set_result.ok);

        
        sleep(Duration::from_millis(100)).await;

        
        let get_result = client2.get("key1").await.unwrap();
        
        
        assert_eq!(get_result.value, "value1", "Consistency violation: client2 cannot read value written by client1");

        
        let (key, value1, value2) = ("key2", "value1", "value2");
        
        
        let write1 = client1.set(key, value1);
        let write2 = client2.set(key, value2);
        
        let (result1, result2) = tokio::join!(write1, write2);
        assert!(result1.is_ok() && result2.is_ok());

        
        let get_result1 = client1.get(key).await.unwrap();
        let get_result2 = client2.get(key).await.unwrap();

        
        
        assert_eq!(
            get_result1.value, 
            get_result2.value,
            "Consistency violation: different values returned from different nodes"
        );

        
        let key = "key3";
        
        
        client1.set(key, "initial_value").await.unwrap();
        
        
        sleep(Duration::from_secs(1)).await;
        
        
        client2.set(key, "new_value").await.unwrap();
        
        
        let value1 = client1.get(key).await.unwrap();
        let value2 = client2.get(key).await.unwrap();
        
        
        assert_eq!(
            value1.value,
            value2.value,
            "Consistency violation: values diverged after leader change"
        );
    }

    #[tokio::test]
    async fn test_linearizability_violation() {
        let (mut client1, mut client2) = setup_clients().await;
        let key = "linearizability_test";

        
        client1.set(key, "initial").await.unwrap();

        
        let read1 = client1.get(key).await.unwrap();
        assert_eq!(read1.value, "initial");

        
        client2.set(key, "updated").await.unwrap();

        
        let read2 = client1.get(key).await.unwrap();
        
        
        assert_eq!(
            read2.value,
            "updated",
            "Linearizability violation: read after write returned stale value"
        );
    }
} 
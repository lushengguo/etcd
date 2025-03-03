mod consistency_tests {
    use crate::client::Client;
    use std::time::Duration;
    use tokio::time::sleep;
    use std::error::Error;

    async fn setup_clients() -> Result<(Client, Client), Box<dyn Error>> {
        let client1 = Client::connect("127.0.0.1:2379").await?;
        let client2 = Client::connect("127.0.0.1:2380").await?;
        Ok((client1, client2))
    }

    #[tokio::test]
    async fn test_consistency_violation() -> Result<(), Box<dyn Error>> {
        // 这里应该延迟一点时间，确保服务器已经启动
        sleep(Duration::from_millis(500)).await;
        
        let mut client1 = Client::connect("127.0.0.1:2379").await?;
        let mut client2 = Client::connect("127.0.0.1:2380").await?;
        
        // 测试场景1：网络分区导致的不一致
        // 在client1上写入数据
        let set_result = client1.set("key1", "value1").await?;
        assert!(set_result.ok);

        // 模拟网络延迟
        sleep(Duration::from_millis(100)).await;

        // 在client2上读取数据，可能读到旧值
        let get_result = client2.get("key1").await?;
        
        // 这里应该失败，因为在没有proper raft实现的情况下，可能读到旧值或空值
        assert_eq!(get_result.value, "value1", "Consistency violation: client2 cannot read value written by client1");

        // 测试场景2：并发写入冲突
        let (key, value1, value2) = ("key2", "value1", "value2");
        
        // 并发写入相同的key
        let write1 = client1.set(key, value1);
        let write2 = client2.set(key, value2);
        
        let (result1, result2) = tokio::join!(write1, write2);
        assert!(result1.is_ok() && result2.is_ok());

        // 读取最终值
        let get_result1 = client1.get(key).await?;
        let get_result2 = client2.get(key).await?;

        // 在正确的实现中，这两个值应该相同
        // 但在当前实现中，可能会出现不一致
        assert_eq!(
            get_result1.value, 
            get_result2.value,
            "Consistency violation: different values returned from different nodes"
        );

        // 测试场景3：Leader宕机后的一致性
        let key = "key3";
        
        // 在client1上写入数据
        client1.set(key, "initial_value").await?;
        
        // 模拟leader宕机和选举延迟
        sleep(Duration::from_secs(1)).await;
        
        // 在新的leader上写入新值
        client2.set(key, "new_value").await?;
        
        // 在两个客户端上读取值
        let value1 = client1.get(key).await?;
        let value2 = client2.get(key).await?;
        
        // 在正确的raft实现中，这两个值应该相同
        assert_eq!(
            value1.value,
            value2.value,
            "Consistency violation: values diverged after leader change"
        );
        
        Ok(())
    }

    #[tokio::test]
    async fn test_linearizability_violation() -> Result<(), Box<dyn Error>> {
        let (mut client1, mut client2) = setup_clients().await?;
        let key = "linearizability_test";

        // 写入初始值
        client1.set(key, "initial").await?;

        // 客户端1读取值
        let read1 = client1.get(key).await?;
        assert_eq!(read1.value, "initial");

        // 客户端2更新值
        client2.set(key, "updated").await?;

        // 客户端1再次读取
        let read2 = client1.get(key).await?;
        
        // 在线性一致性下，第二次读取必须看到更新后的值
        assert_eq!(
            read2.value,
            "updated",
            "Linearizability violation: read after write returned stale value"
        );
        
        Ok(())
    }
} 
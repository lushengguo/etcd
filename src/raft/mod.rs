mod command;
mod log;
pub mod node;
pub mod raft_client;
pub mod raft_service;
pub mod rpc;
mod state;
pub mod testing;

#[cfg(test)]
mod tests {
    use super::testing::*;
    use tokio::time::timeout;
    use std::time::Duration;
    use log::info;

    const TEST_TIMEOUT: Duration = Duration::from_secs(30);
    const ELECTION_TIMEOUT: Duration = Duration::from_secs(5);
    const STABILITY_CHECK_INTERVAL: Duration = Duration::from_millis(200);

    #[tokio::test]
    async fn test_basic_election() {
        let result = timeout(TEST_TIMEOUT, async {
            info!("开始基本选举测试");
            
            let mut config = TestClusterConfig::default();
            config.network_delay_ms = 20;
            
            let mut cluster = TestCluster::new(config).await;
            cluster.start().await;

            tokio::time::sleep(Duration::from_secs(1)).await;

            info!("等待选举领导者...");
            let leader = cluster.wait_for_leader(ELECTION_TIMEOUT.as_millis() as u64).await;
            assert!(leader.is_some(), "未能在超时时间内选出领导者");

            let leader_idx = leader.unwrap();
            info!("节点 {} 被选为领导者", leader_idx + 1);

            let status = cluster.get_status_summary().await;
            info!("集群状态: {}", status);

            assert!(cluster.check_single_leader().await, "集群中存在多个领导者");
        }).await;

        match result {
            Ok(_) => info!("基本选举测试成功完成"),
            Err(_) => panic!("测试超时"),
        }
    }
}

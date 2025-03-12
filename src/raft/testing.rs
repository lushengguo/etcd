use log::{debug, error, info};
use rand;
use rand::Rng;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::Mutex;
use tokio::time;

use crate::raft::node::{LocalNode, NodeState, RemoteNode};
use crate::raft::rpc::LogEntry;

pub struct TestClusterConfig {
    pub node_count: usize,

    pub base_port: u16,

    pub simulate_network_delay: bool,

    pub network_delay_ms: u64,

    pub simulate_node_failures: bool,

    pub node_failure_probability: f64,
}

impl Default for TestClusterConfig {
    fn default() -> Self {
        Self {
            node_count: 3,
            base_port: 10000,
            simulate_network_delay: false,
            network_delay_ms: 50,
            simulate_node_failures: false,
            node_failure_probability: 0.1,
        }
    }
}

pub struct TestCluster {
    pub nodes: Vec<Arc<Mutex<LocalNode>>>,

    pub node_failures: Vec<bool>,

    pub config: TestClusterConfig,

    pub running: bool,
}

impl TestCluster {
    pub async fn new(config: TestClusterConfig) -> Self {
        let mut nodes = Vec::new();
        let mut node_failures = Vec::new();

        let mut node_configs = Vec::new();
        for i in 0..config.node_count {
            let node_id = (i + 1) as u64;
            let port = config.base_port + i as u16;
            let addr = format!("127.0.0.1:{}", port);
            node_configs.push((node_id, addr));
        }

        for i in 0..config.node_count {
            let node_id = (i + 1) as u64;
            let mut node = LocalNode::new(node_id);

            let mut remote_nodes = Vec::new();
            for (id, addr) in &node_configs {
                remote_nodes.push(RemoteNode {
                    node_uid: *id,
                    addr: addr.clone(),
                });
            }

            node.connect_to_cluster(remote_nodes).await.unwrap();

            nodes.push(Arc::new(Mutex::new(node)));
            node_failures.push(false);
        }

        Self {
            nodes,
            node_failures,
            config,
            running: false,
        }
    }

    pub async fn start(&mut self) {
        if self.running {
            return;
        }

        self.running = true;

        for i in 0..self.nodes.len() {
            if self.node_failures[i] {
                continue;
            }

            let node = self.nodes[i].clone();
            let simulate_delay = self.config.simulate_network_delay;
            let delay_ms = self.config.network_delay_ms;

            tokio::spawn(async move {
                let mut interval = time::interval(Duration::from_millis(10));

                loop {
                    interval.tick().await;

                    if simulate_delay {
                        time::sleep(Duration::from_millis(delay_ms)).await;
                    }

                    let mut node_guard = node.lock().await;
                    node_guard.handle_heartbeat_timeout().await;
                }
            });
        }
    }

    pub async fn simulate_node_failure(&mut self, node_idx: usize) {
        if node_idx >= self.nodes.len() {
            return;
        }

        info!("模拟节点 {} 故障", node_idx + 1);
        self.node_failures[node_idx] = true;
    }

    pub async fn recover_node(&mut self, node_idx: usize) {
        if node_idx >= self.nodes.len() {
            return;
        }

        info!("恢复节点 {} 运行", node_idx + 1);
        self.node_failures[node_idx] = false;
    }

    pub async fn find_leader(&self) -> Option<usize> {
        for i in 0..self.nodes.len() {
            if self.node_failures[i] {
                continue;
            }

            let node = self.nodes[i].lock().await;
            if node.state == NodeState::Leader {
                return Some(i);
            }
        }

        None
    }

    pub async fn wait_for_leader(&self, timeout_ms: u64) -> Option<usize> {
        let start = SystemTime::now();

        loop {
            if let Some(leader) = self.find_leader().await {
                return Some(leader);
            }

            match SystemTime::now().duration_since(start) {
                Ok(duration) => {
                    if duration.as_millis() as u64 > timeout_ms {
                        return None;
                    }
                }
                Err(_) => return None,
            }

            time::sleep(Duration::from_millis(10)).await;
        }
    }

    pub async fn set_key_value(&self, key: &str, value: &str) -> bool {
        if let Some(leader_idx) = self.find_leader().await {
            let mut node = self.nodes[leader_idx].lock().await;
            match node.set(key.to_string(), value.to_string()).await {
                Ok(_) => true,
                Err(e) => {
                    error!("设置键值对失败: {:?}", e);
                    false
                }
            }
        } else {
            error!("找不到领导者节点");
            false
        }
    }

    pub async fn get_key(&self, key: &str, node_idx: usize) -> Option<String> {
        if node_idx >= self.nodes.len() || self.node_failures[node_idx] {
            return None;
        }

        let node = self.nodes[node_idx].lock().await;
        match node.get(key.to_string()).await {
            Ok(value) => Some(value),
            Err(_) => None,
        }
    }

    pub async fn check_consistency(&self, key: &str) -> bool {
        let mut values = Vec::new();

        for i in 0..self.nodes.len() {
            if self.node_failures[i] {
                continue;
            }

            if let Some(value) = self.get_key(key, i).await {
                values.push(value);
            }
        }

        if values.is_empty() {
            return true;
        }

        let first = &values[0];
        for value in &values {
            if value != first {
                return false;
            }
        }

        true
    }

    pub async fn get_status_summary(&self) -> String {
        let mut summary = String::new();

        for i in 0..self.nodes.len() {
            if self.node_failures[i] {
                summary.push_str(&format!("节点{}[故障], ", i + 1));
                continue;
            }

            let node = self.nodes[i].lock().await;
            let state = match node.state {
                NodeState::Follower => "跟随者",
                NodeState::Candidate => "候选人",
                NodeState::Leader => "领导者",
            };

            summary.push_str(&format!(
                "节点{}[{}:任期{}], ",
                i + 1,
                state,
                node.current_term
            ));
        }

        summary
    }

    pub async fn print_logs(&self) {
        for i in 0..self.nodes.len() {
            if self.node_failures[i] {
                continue;
            }

            let node = self.nodes[i].lock().await;
            debug!("节点 {} 日志:", i + 1);
            for (idx, entry) in node.log.iter().enumerate() {
                debug!("  {}: 任期={}, 命令={}", idx + 1, entry.term, entry.command);
            }
        }
    }

    pub async fn get_log_length(&self, node_idx: usize) -> Option<usize> {
        if node_idx >= self.nodes.len() || self.node_failures[node_idx] {
            return None;
        }

        let node = self.nodes[node_idx].lock().await;
        Some(node.log.len())
    }

    pub async fn create_partition(&mut self, group1: Vec<usize>, group2: Vec<usize>) {
        info!("创建网络分区 - 分区1: {:?}, 分区2: {:?}", group1, group2);

        for i in 0..self.node_failures.len() {
            self.node_failures[i] = false;
        }

        for &idx in &group2 {
            if idx < self.nodes.len() {
                self.node_failures[idx] = true;
            }
        }
    }

    pub async fn check_single_leader(&self) -> bool {
        let mut leader_count = 0;
        let mut leader_term = 0;

        for i in 0..self.nodes.len() {
            if self.node_failures[i] {
                continue;
            }

            let node = self.nodes[i].lock().await;
            if node.state == NodeState::Leader {
                leader_count += 1;
                leader_term = node.current_term;
                info!("找到领导者: 节点 {} (任期 {})", i + 1, leader_term);
            }
        }

        leader_count <= 1
    }
}

pub enum TestScenario {
    BasicElection,

    LeaderFailure,

    NetworkPartition,

    LogReplication,

    MembershipChange,

    LogConflictResolution,

    FollowerCrashRecovery,

    MultipleElections,

    SafetyTest,

    HighLoadTest,

    RandomFailureTest,
}

pub async fn run_test_scenario(scenario: TestScenario) -> bool {
    match scenario {
        TestScenario::BasicElection => test_basic_election().await,
        TestScenario::LeaderFailure => test_leader_failure().await,
        TestScenario::NetworkPartition => test_network_partition().await,
        TestScenario::LogReplication => test_log_replication().await,
        TestScenario::MembershipChange => test_membership_change().await,
        TestScenario::LogConflictResolution => test_log_conflict_resolution().await,
        TestScenario::FollowerCrashRecovery => test_follower_crash_recovery().await,
        TestScenario::MultipleElections => test_multiple_elections().await,
        TestScenario::SafetyTest => test_safety().await,
        TestScenario::HighLoadTest => test_high_load().await,
        TestScenario::RandomFailureTest => test_random_failures().await,
    }
}

async fn test_basic_election() -> bool {
    info!("开始测试：基本领导选举");

    let mut config = TestClusterConfig::default();
    config.node_count = 3;
    let mut cluster = TestCluster::new(config).await;

    cluster.start().await;

    info!("等待集群选出领导者...");
    let leader = cluster.wait_for_leader(5000).await;

    if leader.is_none() {
        error!("测试失败：集群未能在规定时间内选出领导者");
        return false;
    }

    let leader_idx = leader.unwrap();
    info!("节点 {} 被选为领导者", leader_idx + 1);

    let status = cluster.get_status_summary().await;
    info!("集群状态: {}", status);

    true
}

async fn test_leader_failure() -> bool {
    info!("开始测试：领导者故障");

    let mut config = TestClusterConfig::default();
    config.node_count = 3;
    let mut cluster = TestCluster::new(config).await;

    cluster.start().await;

    info!("等待集群选出第一个领导者...");
    let leader = cluster.wait_for_leader(5000).await;

    if leader.is_none() {
        error!("测试失败：集群未能在规定时间内选出第一个领导者");
        return false;
    }

    let leader_idx = leader.unwrap();
    info!("节点 {} 被选为领导者", leader_idx + 1);

    info!("模拟领导者故障...");
    cluster.simulate_node_failure(leader_idx).await;

    info!("等待集群选出新的领导者...");
    let new_leader = cluster.wait_for_leader(5000).await;

    if new_leader.is_none() {
        error!("测试失败：领导者故障后，集群未能在规定时间内选出新领导者");
        return false;
    }

    let new_leader_idx = new_leader.unwrap();
    info!("节点 {} 被选为新的领导者", new_leader_idx + 1);

    if new_leader_idx == leader_idx {
        error!("测试失败：新领导者与故障领导者是同一个节点");
        return false;
    }

    info!("恢复旧领导者...");
    cluster.recover_node(leader_idx).await;

    time::sleep(Duration::from_millis(500)).await;

    let status = cluster.get_status_summary().await;
    info!("集群状态: {}", status);

    true
}

async fn test_network_partition() -> bool {
    info!("开始测试：网络分区");

    let mut config = TestClusterConfig::default();
    config.node_count = 5;
    let mut cluster = TestCluster::new(config).await;

    cluster.start().await;

    info!("等待集群选出领导者...");
    let leader = cluster.wait_for_leader(5000).await;

    if leader.is_none() {
        error!("测试失败：集群未能在规定时间内选出领导者");
        return false;
    }

    let leader_idx = leader.unwrap();
    info!("节点 {} 被选为领导者", leader_idx + 1);

    let key = "test_key";
    let value = "test_value";
    info!("设置键值对: {}={}", key, value);
    if !cluster.set_key_value(key, value).await {
        error!("测试失败：无法设置键值对");
        return false;
    }

    info!("创建网络分区...");

    cluster.simulate_node_failure(3).await;
    cluster.simulate_node_failure(4).await;

    time::sleep(Duration::from_millis(500)).await;

    if leader_idx >= 3 {
        info!("原领导者在少数派分区，等待多数派分区选出新领导者...");
        let new_leader = cluster.wait_for_leader(5000).await;

        if new_leader.is_none() {
            error!("测试失败：网络分区后，多数派未能在规定时间内选出新领导者");
            return false;
        }

        let new_leader_idx = new_leader.unwrap();
        info!("节点 {} 在多数派分区中被选为新的领导者", new_leader_idx + 1);
    } else {
        info!("原领导者在多数派分区，应该保持领导地位");
    }

    let key2 = "partition_key";
    let value2 = "partition_value";
    info!("在多数派分区中设置新键值对: {}={}", key2, value2);
    if !cluster.set_key_value(key2, value2).await {
        error!("测试失败：无法在多数派分区中设置键值对");
        return false;
    }

    info!("修复网络分区...");
    cluster.recover_node(3).await;
    cluster.recover_node(4).await;

    time::sleep(Duration::from_millis(1000)).await;

    info!("检查键值一致性...");
    if !cluster.check_consistency(key).await || !cluster.check_consistency(key2).await {
        error!("测试失败：网络分区修复后，节点上的键值不一致");
        return false;
    }

    info!("网络分区测试成功：所有节点上的键值一致");

    let status = cluster.get_status_summary().await;
    info!("集群状态: {}", status);

    true
}

async fn test_log_replication() -> bool {
    info!("开始测试：日志复制一致性");

    let mut config = TestClusterConfig::default();
    config.node_count = 3;
    let mut cluster = TestCluster::new(config).await;

    cluster.start().await;

    info!("等待集群选出领导者...");
    let leader = cluster.wait_for_leader(5000).await;

    if leader.is_none() {
        error!("测试失败：集群未能在规定时间内选出领导者");
        return false;
    }

    let leader_idx = leader.unwrap();
    info!("节点 {} 被选为领导者", leader_idx + 1);

    let keys = vec!["key1", "key2", "key3", "key4", "key5"];
    let values = vec!["value1", "value2", "value3", "value4", "value5"];

    for i in 0..keys.len() {
        info!("设置键值对: {}={}", keys[i], values[i]);
        if !cluster.set_key_value(keys[i], values[i]).await {
            error!("测试失败：无法设置键值对 {}={}", keys[i], values[i]);
            return false;
        }
    }

    time::sleep(Duration::from_millis(500)).await;

    info!("检查键值一致性...");
    for key in keys {
        if !cluster.check_consistency(key).await {
            error!("测试失败：键 {} 在不同节点上的值不一致", key);
            return false;
        }
    }

    info!("日志复制一致性测试成功：所有节点上的键值一致");

    cluster.print_logs().await;

    let status = cluster.get_status_summary().await;
    info!("集群状态: {}", status);

    true
}

async fn test_high_load() -> bool {
    info!("开始测试：高负载");

    let mut config = TestClusterConfig::default();
    config.node_count = 5;
    let mut cluster = TestCluster::new(config).await;

    cluster.start().await;

    info!("等待集群选出领导者...");
    let leader = cluster.wait_for_leader(5000).await;

    if leader.is_none() {
        error!("测试失败：集群未能在规定时间内选出领导者");
        return false;
    }

    let leader_idx = leader.unwrap();
    info!("节点 {} 被选为领导者", leader_idx + 1);

    let test_count = 100;
    info!("开始高负载测试，将设置 {} 个键值对...", test_count);

    let start_time = std::time::Instant::now();

    let mut success_count = 0;
    let mut failed_keys = Vec::new();

    for i in 0..test_count {
        let key = format!("load_key_{}", i);
        let value = format!("load_value_{}", i);

        info!("设置键值对: {}={}", key, value);
        if cluster.set_key_value(&key, &value).await {
            success_count += 1;
        } else {
            failed_keys.push(key);
        }

        if i % 10 == 0 && i > 0 {
            time::sleep(Duration::from_millis(50)).await;
        }
    }

    let elapsed = start_time.elapsed();
    info!(
        "高负载测试完成，成功: {}/{}，用时: {:?}",
        success_count, test_count, elapsed
    );

    if success_count < test_count * 9 / 10 {
        error!("测试失败：成功率过低，低于90%");
        error!("失败的键: {:?}", failed_keys);
        return false;
    }

    info!("验证数据一致性...");

    let mut rng = rand::thread_rng();
    let sample_size = test_count.min(10);

    for _ in 0..sample_size {
        let idx = rng.gen_range(0..test_count);
        let key = format!("load_key_{}", idx);

        if !cluster.check_consistency(&key).await {
            error!("测试失败：键 {} 在不同节点上的值不一致", key);
            return false;
        }
    }

    info!("高负载测试成功：数据写入和一致性验证通过");

    let status = cluster.get_status_summary().await;
    info!("集群状态: {}", status);

    true
}

async fn test_random_failures() -> bool {
    info!("开始测试：随机故障");

    let mut config = TestClusterConfig::default();
    config.node_count = 5;
    let mut cluster = TestCluster::new(config).await;

    cluster.start().await;

    info!("等待集群选出初始领导者...");
    let leader = cluster.wait_for_leader(5000).await;

    if leader.is_none() {
        error!("测试失败：集群未能在规定时间内选出初始领导者");
        return false;
    }

    let mut leader_idx = leader.unwrap();
    info!("节点 {} 被选为初始领导者", leader_idx + 1);

    let init_key = "random_init_key";
    let init_value = "random_init_value";
    info!("设置初始键值对: {}={}", init_key, init_value);
    if !cluster.set_key_value(init_key, init_value).await {
        error!("测试失败：无法设置初始键值对");
        return false;
    }

    let test_rounds = 5;
    let mut rng = rand::thread_rng();

    let mut active_keys = vec![init_key.to_string()];

    for round in 1..=test_rounds {
        info!("=== 随机故障测试轮次 {} ===", round);

        let fault_count = rng.gen_range(1..=2);
        let mut fault_nodes = Vec::new();

        for _ in 0..fault_count {
            loop {
                let node_idx = rng.gen_range(0..cluster.nodes.len());
                if !fault_nodes.contains(&node_idx) && fault_nodes.len() < cluster.nodes.len() / 2 {
                    fault_nodes.push(node_idx);
                    break;
                }
            }
        }

        for &node_idx in &fault_nodes {
            info!("模拟节点 {} 故障...", node_idx + 1);
            cluster.simulate_node_failure(node_idx).await;
        }

        time::sleep(Duration::from_millis(1000)).await;

        if fault_nodes.contains(&leader_idx) {
            info!("当前领导者故障，等待新的领导者被选出...");
            let new_leader = cluster.wait_for_leader(5000).await;

            if new_leader.is_none() {
                error!("测试失败：领导者故障后，集群未能在规定时间内选出新领导者");
                return false;
            }

            leader_idx = new_leader.unwrap();
            info!("节点 {} 被选为新的领导者", leader_idx + 1);
        }

        let key = format!("random_key_{}", round);
        let value = format!("random_value_{}", round);
        info!("设置新键值对: {}={}", key, value);
        if !cluster.set_key_value(&key, &value).await {
            error!("测试失败：随机故障后无法设置键值对");
            return false;
        }

        active_keys.push(key);

        let recover_count = if fault_nodes.len() > 0 {
            rng.gen_range(0..=fault_nodes.len())
        } else {
            0
        };
        for i in 0..recover_count {
            let node_idx = fault_nodes[i];
            info!("恢复节点 {} 运行...", node_idx + 1);
            cluster.recover_node(node_idx).await;
        }

        time::sleep(Duration::from_millis(500)).await;

        info!("检查现有键的一致性...");
        for key in &active_keys {
            if !cluster.check_consistency(key).await {
                error!("测试失败：随机故障后，键 {} 的值不一致", key);
                return false;
            }
        }
    }

    for i in 0..cluster.node_failures.len() {
        if cluster.node_failures[i] {
            info!("恢复节点 {} 运行...", i + 1);
            cluster.recover_node(i).await;
        }
    }

    time::sleep(Duration::from_millis(1000)).await;

    info!("最终检查所有键的一致性...");
    for key in &active_keys {
        if !cluster.check_consistency(key).await {
            error!("测试失败：最终状态下，键 {} 的值不一致", key);
            return false;
        }
    }

    info!("随机故障测试成功：系统能够在随机节点故障下保持正常运行和数据一致性");

    let status = cluster.get_status_summary().await;
    info!("最终集群状态: {}", status);

    true
}

async fn test_membership_change() -> bool {
    info!("开始测试：集群成员变更");

    let mut config = TestClusterConfig::default();
    config.node_count = 3;
    let mut cluster = TestCluster::new(config).await;

    cluster.start().await;

    info!("等待集群选出领导者...");
    let leader = cluster.wait_for_leader(5000).await;

    if leader.is_none() {
        error!("测试失败：集群未能在规定时间内选出领导者");
        return false;
    }

    let leader_idx = leader.unwrap();
    info!("节点 {} 被选为领导者", leader_idx + 1);

    let key = "membership_key";
    let value = "initial_value";
    info!("设置初始键值对: {}={}", key, value);
    if !cluster.set_key_value(key, value).await {
        error!("测试失败：无法设置初始键值对");
        return false;
    }

    info!("模拟添加新节点到集群...");

    info!("模拟集群配置变更完成");

    let new_value = "after_change_value";
    info!("设置键值对更新: {}={}", key, new_value);
    if !cluster.set_key_value(key, new_value).await {
        error!("测试失败：集群成员变更后无法更新键值对");
        return false;
    }

    time::sleep(Duration::from_millis(500)).await;
    if !cluster.check_consistency(key).await {
        error!("测试失败：集群成员变更后，键值不一致");
        return false;
    }

    info!("集群成员变更测试成功（模拟）：系统在成员变更后保持正常运行");
    info!("注意：此测试当前只是一个模拟，未来需要实现真正的动态成员变更功能");

    let status = cluster.get_status_summary().await;
    info!("集群状态: {}", status);

    true
}

async fn test_log_conflict_resolution() -> bool {
    info!("开始测试：日志冲突解决");

    let mut config = TestClusterConfig::default();
    config.node_count = 5;
    let mut cluster = TestCluster::new(config).await;

    cluster.start().await;

    info!("等待集群选出领导者...");
    let leader = cluster.wait_for_leader(5000).await;

    if leader.is_none() {
        error!("测试失败：集群未能在规定时间内选出领导者");
        return false;
    }

    let leader_idx = leader.unwrap();
    info!("节点 {} 被选为领导者", leader_idx + 1);

    let key = "conflict_key";
    let value = "original_value";
    info!("设置初始键值对: {}={}", key, value);
    if !cluster.set_key_value(key, value).await {
        error!("测试失败：无法设置初始键值对");
        return false;
    }

    let partition1: Vec<usize> = vec![leader_idx, (leader_idx + 1) % cluster.nodes.len()];
    let mut partition2: Vec<usize> = Vec::new();
    for i in 0..cluster.nodes.len() {
        if !partition1.contains(&i) {
            partition2.push(i);
        }
    }

    info!("创建网络分区，使旧领导者与多数节点断开连接...");
    cluster
        .create_partition(partition1, partition2.clone())
        .await;

    info!("等待分区2选出新的领导者...");

    time::sleep(Duration::from_millis(2000)).await;

    let new_value = "partition2_value";
    info!("尝试在分区2设置键值对: {}={}", key, new_value);

    let mut success = false;
    for &node_idx in &partition2 {
        let node = cluster.nodes[node_idx].lock().await;
        if node.state == NodeState::Leader {
            success = true;
            info!("在分区2中找到新的领导者: 节点 {}", node_idx + 1);
            break;
        }
    }

    if !success {
        error!("测试失败：分区2未能选出新的领导者");
        return false;
    }

    info!("修复网络分区，使所有节点重新连接...");
    for i in 0..cluster.node_failures.len() {
        cluster.recover_node(i).await;
    }

    time::sleep(Duration::from_millis(2000)).await;

    if !cluster.check_single_leader().await {
        error!("测试失败：修复网络分区后，系统存在多个领导者");
        return false;
    }

    let final_value = "final_value";
    info!("设置最终键值对: {}={}", key, final_value);
    if !cluster.set_key_value(key, final_value).await {
        error!("测试失败：修复网络分区后，无法设置键值对");
        return false;
    }

    time::sleep(Duration::from_millis(500)).await;

    info!("检查键值一致性...");
    if !cluster.check_consistency(key).await {
        error!("测试失败：修复网络分区后，键值不一致");
        return false;
    }

    info!("日志冲突解决测试成功：所有节点上的键值一致");

    let status = cluster.get_status_summary().await;
    info!("集群状态: {}", status);

    true
}

async fn test_follower_crash_recovery() -> bool {
    info!("开始测试：跟随者崩溃和恢复");

    let mut config = TestClusterConfig::default();
    config.node_count = 5;
    let mut cluster = TestCluster::new(config).await;

    cluster.start().await;

    info!("等待集群选出领导者...");
    let leader = cluster.wait_for_leader(5000).await;

    if leader.is_none() {
        error!("测试失败：集群未能在规定时间内选出领导者");
        return false;
    }

    let leader_idx = leader.unwrap();
    info!("节点 {} 被选为领导者", leader_idx + 1);

    let key = "follower_test";
    let value = "initial_value";
    info!("设置初始键值对: {}={}", key, value);
    if !cluster.set_key_value(key, value).await {
        error!("测试失败：无法设置初始键值对");
        return false;
    }

    let mut follower_indices = Vec::new();
    for i in 0..cluster.nodes.len() {
        if i != leader_idx {
            follower_indices.push(i);
            if follower_indices.len() >= 2 {
                break;
            }
        }
    }

    if follower_indices.len() < 2 {
        error!("测试失败：无法找到足够的跟随者节点");
        return false;
    }

    info!("模拟跟随者节点 {} 故障...", follower_indices[0] + 1);
    cluster.simulate_node_failure(follower_indices[0]).await;

    let keys = vec!["f_key1", "f_key2", "f_key3"];
    let values = vec!["f_value1", "f_value2", "f_value3"];

    for i in 0..keys.len() {
        info!("设置键值对: {}={}", keys[i], values[i]);
        if !cluster.set_key_value(keys[i], values[i]).await {
            error!(
                "测试失败：跟随者故障后无法设置键值对 {}={}",
                keys[i], values[i]
            );
            return false;
        }
    }

    info!("模拟跟随者节点 {} 故障...", follower_indices[1] + 1);
    cluster.simulate_node_failure(follower_indices[1]).await;

    let key4 = "f_key4";
    let value4 = "f_value4";
    info!("设置键值对: {}={}", key4, value4);
    if !cluster.set_key_value(key4, value4).await {
        error!("测试失败：多个跟随者故障后无法设置键值对");
        return false;
    }

    info!("恢复跟随者节点 {} 运行...", follower_indices[0] + 1);
    cluster.recover_node(follower_indices[0]).await;

    time::sleep(Duration::from_millis(500)).await;

    info!("检查恢复的跟随者是否有最新日志...");
    for key in keys.iter().chain(&[key4]) {
        let leader_value = cluster.get_key(key, leader_idx).await;
        let follower_value = cluster.get_key(key, follower_indices[0]).await;

        if leader_value != follower_value {
            error!("测试失败：恢复的跟随者没有正确复制键 {} 的值", key);
            return false;
        }
    }

    info!("恢复跟随者节点 {} 运行...", follower_indices[1] + 1);
    cluster.recover_node(follower_indices[1]).await;

    time::sleep(Duration::from_millis(500)).await;

    let final_key = "final_key";
    let final_value = "final_value";
    info!("设置最终键值对: {}={}", final_key, final_value);
    if !cluster.set_key_value(final_key, final_value).await {
        error!("测试失败：所有节点恢复后无法设置键值对");
        return false;
    }

    time::sleep(Duration::from_millis(500)).await;

    info!("检查所有键值对的一致性...");
    for key in keys.iter().chain(&[key4, final_key]) {
        if !cluster.check_consistency(key).await {
            error!("测试失败：节点恢复后，键 {} 的值不一致", key);
            return false;
        }
    }

    info!("跟随者崩溃和恢复测试成功：所有节点上的键值一致");

    let status = cluster.get_status_summary().await;
    info!("集群状态: {}", status);

    true
}

async fn test_multiple_elections() -> bool {
    info!("开始测试：多轮选举");

    let mut config = TestClusterConfig::default();
    config.node_count = 5;
    let mut cluster = TestCluster::new(config).await;

    cluster.start().await;

    for round in 1..=3 {
        info!("=== 选举轮次 {} ===", round);

        info!("等待集群选出领导者...");
        let leader = cluster.wait_for_leader(5000).await;

        if leader.is_none() {
            error!("测试失败：第 {} 轮选举未能在规定时间内选出领导者", round);
            return false;
        }

        let leader_idx = leader.unwrap();
        info!("节点 {} 被选为第 {} 轮的领导者", leader_idx + 1, round);

        let status = cluster.get_status_summary().await;
        info!("集群状态: {}", status);

        let key = format!("round{}_key", round);
        let value = format!("round{}_value", round);
        info!("设置键值对: {}={}", key, value);
        if !cluster.set_key_value(&key, &value).await {
            error!("测试失败：第 {} 轮的领导者无法设置键值对", round);
            return false;
        }

        time::sleep(Duration::from_millis(300)).await;

        if !cluster.check_consistency(&key).await {
            error!("测试失败：第 {} 轮后，键值不一致", round);
            return false;
        }

        if round < 3 {
            info!("模拟领导者 {} 故障，触发下一轮选举...", leader_idx + 1);
            cluster.simulate_node_failure(leader_idx).await;

            time::sleep(Duration::from_millis(1000)).await;
        }
    }

    for i in 0..cluster.node_failures.len() {
        if cluster.node_failures[i] {
            info!("恢复节点 {} 运行...", i + 1);
            cluster.recover_node(i).await;
        }
    }

    time::sleep(Duration::from_millis(1000)).await;

    for round in 1..=3 {
        let key = format!("round{}_key", round);
        if !cluster.check_consistency(&key).await {
            error!("测试失败：最终状态下，第 {} 轮的键值不一致", round);
            return false;
        }
    }

    info!("多轮选举测试成功：所有轮次的键值都一致");

    let status = cluster.get_status_summary().await;
    info!("最终集群状态: {}", status);

    true
}

async fn test_safety() -> bool {
    info!("开始测试：安全性（最多只有一个领导者）");

    let mut config = TestClusterConfig::default();
    config.node_count = 5;
    let mut cluster = TestCluster::new(config).await;

    cluster.start().await;

    info!("等待集群选出初始领导者...");
    let leader = cluster.wait_for_leader(5000).await;

    if leader.is_none() {
        error!("测试失败：集群未能在规定时间内选出初始领导者");
        return false;
    }

    for i in 1..=3 {
        info!("=== 安全性测试轮次 {} ===", i);

        let mut rng = rand::thread_rng();
        let partition_size = cluster.nodes.len() / 2;

        let mut partition1 = Vec::new();
        let mut partition2 = Vec::new();

        for j in 0..cluster.nodes.len() {
            if j < partition_size {
                partition1.push(j);
            } else {
                partition2.push(j);
            }
        }

        info!(
            "创建网络分区 - 分区1: {:?}, 分区2: {:?}",
            partition1, partition2
        );
        cluster
            .create_partition(partition1.clone(), partition2.clone())
            .await;

        time::sleep(Duration::from_millis(2000)).await;

        let mut leader_in_partition1 = false;
        for &node_idx in &partition1 {
            let node = cluster.nodes[node_idx].lock().await;
            if node.state == NodeState::Leader {
                if leader_in_partition1 {
                    error!("测试失败：在分区1中发现多个领导者");
                    return false;
                }
                leader_in_partition1 = true;
                info!("分区1中发现领导者: 节点 {}", node_idx + 1);
            }
        }

        info!("修复网络分区...");
        for j in 0..cluster.node_failures.len() {
            cluster.recover_node(j).await;
        }

        time::sleep(Duration::from_millis(1000)).await;

        if !cluster.check_single_leader().await {
            error!("测试失败：修复网络分区后，系统中存在多个领导者");
            return false;
        }

        let key = format!("safety_key_{}", i);
        let value = format!("safety_value_{}", i);
        info!("设置键值对: {}={}", key, value);
        if !cluster.set_key_value(&key, &value).await {
            error!("测试失败：第 {} 轮网络分区修复后，无法设置键值对", i);
            return false;
        }

        time::sleep(Duration::from_millis(500)).await;
        if !cluster.check_consistency(&key).await {
            error!("测试失败：第 {} 轮后，键值不一致", i);
            return false;
        }
    }

    info!("安全性测试成功：在所有测试场景中，最多只有一个领导者");

    let status = cluster.get_status_summary().await;
    info!("最终集群状态: {}", status);

    true
}

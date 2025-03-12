use env_logger::Builder;
use log::{error, info};
use std::env;
use std::io::Write;

use etcd::raft::testing::{run_test_scenario, TestScenario};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    let args: Vec<String> = env::args().collect();

    let scenario = if args.len() > 1 {
        match args[1].as_str() {
            "election" => TestScenario::BasicElection,
            "leader_failure" => TestScenario::LeaderFailure,
            "network_partition" => TestScenario::NetworkPartition,
            "log_replication" => TestScenario::LogReplication,
            "membership_change" => TestScenario::MembershipChange,
            "log_conflict" => TestScenario::LogConflictResolution,
            "follower_crash" => TestScenario::FollowerCrashRecovery,
            "multiple_elections" => TestScenario::MultipleElections,
            "safety" => TestScenario::SafetyTest,
            "high_load" => TestScenario::HighLoadTest,
            "random_failures" => TestScenario::RandomFailureTest,
            "all" => {
                run_all_tests().await?;
                return Ok(());
            }
            _ => {
                println!("未知的测试场景: {}", args[1]);
                println!("可用的测试场景: election, leader_failure, network_partition, log_replication, membership_change, log_conflict, follower_crash, multiple_elections, safety, high_load, random_failures, all");
                return Ok(());
            }
        }
    } else {
        println!("使用方法: cargo run --bin raft_test [测试场景]");
        println!("可用的测试场景: election, leader_failure, network_partition, log_replication, membership_change, log_conflict, follower_crash, multiple_elections, safety, high_load, random_failures, all");
        return Ok(());
    };

    info!("开始运行测试场景...");
    if run_test_scenario(scenario).await {
        info!("测试成功！");
    } else {
        error!("测试失败！");
    }

    Ok(())
}

async fn run_all_tests() -> Result<(), Box<dyn std::error::Error>> {
    let scenarios = vec![
        (TestScenario::BasicElection, "基本领导选举"),
        (TestScenario::LeaderFailure, "领导者故障"),
        (TestScenario::NetworkPartition, "网络分区"),
        (TestScenario::LogReplication, "日志复制一致性"),
        (TestScenario::LogConflictResolution, "日志冲突解决"),
        (TestScenario::FollowerCrashRecovery, "跟随者崩溃恢复"),
        (TestScenario::MultipleElections, "多轮选举"),
        (TestScenario::SafetyTest, "安全性测试"),
        (TestScenario::HighLoadTest, "高负载测试"),
        (TestScenario::RandomFailureTest, "随机故障测试"),
        (TestScenario::MembershipChange, "集群成员变更"),
    ];

    let mut success_count = 0;
    let mut failure_count = 0;

    for (scenario, name) in scenarios {
        info!("===================================================");
        info!("开始测试: {}", name);
        info!("===================================================");

        if run_test_scenario(scenario).await {
            info!("√ 测试 {} 成功", name);
            success_count += 1;
        } else {
            error!("× 测试 {} 失败", name);
            failure_count += 1;
        }

        info!("===================================================");
        info!("");
    }

    info!("测试结果汇总:");
    info!("成功: {}", success_count);
    info!("失败: {}", failure_count);
    info!("总计: {}", success_count + failure_count);

    Ok(())
}

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
                println!("Unknown test scenario: {}", args[1]);
                println!("Available test scenarios: election, leader_failure, network_partition, log_replication, membership_change, log_conflict, follower_crash, multiple_elections, safety, high_load, random_failures, all");
                return Ok(());
            }
        }
    } else {
        println!("Usage: cargo run --bin raft_test [test_scenario]");
        println!("Available test scenarios: election, leader_failure, network_partition, log_replication, membership_change, log_conflict, follower_crash, multiple_elections, safety, high_load, random_failures, all");
        return Ok(());
    };

    info!("Starting test scenario...");
    if run_test_scenario(scenario).await {
        info!("Test successful!");
    } else {
        error!("Test failed!");
    }

    Ok(())
}

async fn run_all_tests() -> Result<(), Box<dyn std::error::Error>> {
    let scenarios = vec![
        (TestScenario::BasicElection, "Basic Leader Election"),
        (TestScenario::LeaderFailure, "Leader Failure"),
        (TestScenario::NetworkPartition, "Network Partition"),
        (TestScenario::LogReplication, "Log Replication Consistency"),
        (TestScenario::LogConflictResolution, "Log Conflict Resolution"),
        (TestScenario::FollowerCrashRecovery, "Follower Crash Recovery"),
        (TestScenario::MultipleElections, "Multiple Elections"),
        (TestScenario::SafetyTest, "Safety Test"),
        (TestScenario::HighLoadTest, "High Load Test"),
        (TestScenario::RandomFailureTest, "Random Failure Test"),
        (TestScenario::MembershipChange, "Membership Change"),
    ];

    let mut success_count = 0;
    let mut failure_count = 0;

    for (scenario, name) in scenarios {
        info!("===================================================");
        info!("Starting test: {}", name);
        info!("===================================================");

        if run_test_scenario(scenario).await {
            info!("√ Test {} successful", name);
            success_count += 1;
        } else {
            error!("× Test {} failed", name);
            failure_count += 1;
        }

        info!("===================================================");
        info!("");
    }

    info!("Test results summary:");
    info!("Success: {}", success_count);
    info!("Failures: {}", failure_count);
    info!("Total: {}", success_count + failure_count);

    Ok(())
}

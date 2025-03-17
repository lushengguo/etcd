use log::info;
use etcd::raft::testing::{TestScenario, run_test_scenario};

#[tokio::main]
async fn main() {
    // 初始化日志系统
    env_logger::init();
    
    info!("=== Starting Raft Tests ===");
    
    // 定义要运行的测试集
    let tests = vec![
        ("BasicElection", TestScenario::BasicElection),
        ("LeaderFailure", TestScenario::LeaderFailure),
        ("NetworkPartition", TestScenario::NetworkPartition),
        ("LogReplication", TestScenario::LogReplication),
        ("MembershipChange", TestScenario::MembershipChange),
        ("LogConflictResolution", TestScenario::LogConflictResolution),
        ("FollowerCrashRecovery", TestScenario::FollowerCrashRecovery),
        ("MultipleElections", TestScenario::MultipleElections),
        ("SafetyTest", TestScenario::SafetyTest),
        ("HighLoadTest", TestScenario::HighLoadTest),
        ("RandomFailureTest", TestScenario::RandomFailureTest),
    ];
    
    let mut passed = 0;
    let mut failed = 0;
    
    // 运行测试
    for (name, scenario) in tests {
        info!("\n\n=== Running Test: {} ===\n", name);
        let result = run_test_scenario(scenario).await;
        
        if result {
            info!("✅ Test {} PASSED", name);
            passed += 1;
        } else {
            info!("❌ Test {} FAILED", name);
            failed += 1;
        }
    }
    
    // 打印测试结果摘要
    info!("\n\n=== Test Results ===");
    info!("Total Tests: {}", passed + failed);
    info!("Passed: {}", passed);
    info!("Failed: {}", failed);
    
    if failed > 0 {
        info!("❌ Some tests failed!");
        std::process::exit(1);
    } else {
        info!("✅ All tests passed!");
    }
} 
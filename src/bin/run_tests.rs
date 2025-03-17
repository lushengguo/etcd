use log::info;
use etcd::raft::testing::{TestScenario, run_test_scenario};

#[tokio::main]
async fn main() {
    // Initialize logging system
    env_logger::init();
    
    info!("=== Starting Raft Tests ===");
    
    // Define test suite to run
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
    
    // Run tests
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
    
    // Print test results summary
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
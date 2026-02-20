use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_full_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("tt").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.arg("init");
    cmd.assert().success();

    // Add Task A
    let mut cmd = Command::cargo_bin("tt").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.args(["add", "Task A"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Created task #1"));

    // Add Task B
    let mut cmd = Command::cargo_bin("tt").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.args(["add", "Task B"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Created task #2"));

    // Add dependency
    let mut cmd = Command::cargo_bin("tt").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.args(["depend", "2", "1"]);
    cmd.assert().success();

    // Set target
    let mut cmd = Command::cargo_bin("tt").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.args(["target", "2"]);
    cmd.assert().success();

    // Next should return Task 1
    let mut cmd = Command::cargo_bin("tt").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.arg("next");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Task A"));

    // Start Task 1
    let mut cmd = Command::cargo_bin("tt").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.args(["start", "1"]);
    cmd.assert().success();

    // Done should fail without DoD
    let mut cmd = Command::cargo_bin("tt").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.arg("done");
    cmd.assert().failure();

    // Set DoD
    let mut cmd = Command::cargo_bin("tt").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.args(["edit", "1", "--dod", "Schema exists"]);
    cmd.assert().success();

    // Now done should succeed
    let mut cmd = Command::cargo_bin("tt").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.arg("done");
    cmd.assert().success();

    // Next should return Task 2
    let mut cmd = Command::cargo_bin("tt").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.arg("next");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Task B"));

    // Set DoD for Task 2
    let mut cmd = Command::cargo_bin("tt").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.args(["edit", "2", "--dod", "Feature works"]);
    cmd.assert().success();

    // Start and complete Task 2
    let mut cmd = Command::cargo_bin("tt").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.args(["start", "2"]);
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("tt").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.arg("done");
    cmd.assert().success();

    // Next should show Target Reached
    let mut cmd = Command::cargo_bin("tt").unwrap();
    cmd.current_dir(&temp_dir);
    cmd.arg("next");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Target reached"));
}

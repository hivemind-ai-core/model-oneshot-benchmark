//! Integration tests for the tt CLI.

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn run_tt(args: &[&str], dir: &Path) -> (String, String, i32) {
    let output = Command::new(env!("CARGO_BIN_EXE_tt"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("Failed to execute tt");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let status = output.status.code().unwrap_or(1);

    (stdout, stderr, status)
}

#[test]
fn test_init_creates_database() {
    let temp = TempDir::new().unwrap();
    let dir = temp.path();

    let (_stdout, _stderr, status) = run_tt(&["init"], dir);
    assert_eq!(status, 0);

    assert!(dir.join("tt.db").exists());
    assert!(dir.join(".tt/artifacts").exists());
}

#[test]
fn test_workflow() {
    let temp = TempDir::new().unwrap();
    let dir = temp.path();

    // Init
    run_tt(&["init"], dir);

    // Add tasks
    let (stdout, _, _) = run_tt(&["add", "Task A"], dir);
    let id_a: i64 = stdout.trim().parse().unwrap();
    assert!(id_a > 0);

    let (stdout, _, _) = run_tt(&["add", "Task B"], dir);
    let id_b: i64 = stdout.trim().parse().unwrap();
    assert!(id_b > 0);

    // Add dependency
    let (stdout, _, status) = run_tt(&["depend", &id_b.to_string(), &id_a.to_string()], dir);
    assert_eq!(status, 0);

    // Set target
    let (_stdout, _stderr, status) = run_tt(&["target", &id_b.to_string()], dir);
    assert_eq!(status, 0);

    // Next should return Task A
    let (stdout, _, _) = run_tt(&["next"], dir);
    assert!(stdout.contains(&id_a.to_string()) || stdout.contains("Task A"));

    // Start Task A
    let (_stdout, _stderr, status) = run_tt(&["start", &id_a.to_string()], dir);
    assert_eq!(status, 0);

    // Done should fail without DoD
    let (_stdout, _stderr, status) = run_tt(&["done"], dir);
    assert_ne!(status, 0);

    // Set DoD
    let (_stdout, _stderr, status) = run_tt(
        &["edit", &id_a.to_string(), "--dod", "Task A complete"],
        dir,
    );
    assert_eq!(status, 0);

    // Complete Task A
    let (_stdout, _stderr, status) = run_tt(&["done"], dir);
    assert_eq!(status, 0);

    // Next should now return Task B
    let (stdout, _, _) = run_tt(&["next"], dir);
    assert!(stdout.contains(&id_b.to_string()) || stdout.contains("Task B"));

    // Set DoD for B
    let (_stdout, _stderr, status) = run_tt(
        &["edit", &id_b.to_string(), "--dod", "Task B complete"],
        dir,
    );
    assert_eq!(status, 0);

    // Start B
    let (_stdout, _stderr, status) = run_tt(&["start", &id_b.to_string()], dir);
    assert_eq!(status, 0);

    // Complete B
    let (_stdout, _stderr, status) = run_tt(&["done"], dir);
    assert_eq!(status, 0);

    // Next should say Target Reached
    let (stdout, _, _) = run_tt(&["next"], dir);
    assert!(stdout.contains("Target Reached"));
}

#[test]
fn test_show_task() {
    let temp = TempDir::new().unwrap();
    let dir = temp.path();

    run_tt(&["init"], dir);

    let (stdout, _, _) = run_tt(
        &["add", "Test task", "--desc", "Description", "--dod", "DoD"],
        dir,
    );
    let id: i64 = stdout.trim().parse().unwrap();

    let (stdout, _, _) = run_tt(&["show", &id.to_string()], dir);
    assert!(stdout.contains("Test task"));
    assert!(stdout.contains("Description"));
    assert!(stdout.contains("DoD"));
}

#[test]
fn test_artifacts() {
    let temp = TempDir::new().unwrap();
    let dir = temp.path();

    run_tt(&["init"], dir);

    let (stdout, _, _) = run_tt(&["add", "Test task", "--dod", "DoD"], dir);
    let id: i64 = stdout.trim().parse().unwrap();

    run_tt(&["start", &id.to_string()], dir);

    // Create a test artifact file
    let artifact_path = dir.join(".tt/artifacts/1-research.md");
    std::fs::create_dir_all(artifact_path.parent().unwrap()).unwrap();
    std::fs::write(&artifact_path, "Research notes").unwrap();

    let (_stdout, _stderr, status) = run_tt(
        &["log", "research", "--file", ".tt/artifacts/1-research.md"],
        dir,
    );
    assert_eq!(status, 0);

    let (stdout, _, _) = run_tt(&["artifacts"], dir);
    assert!(stdout.contains("research"));
}

#[test]
fn test_block_unblock() {
    let temp = TempDir::new().unwrap();
    let dir = temp.path();

    run_tt(&["init"], dir);

    let (stdout, _, _) = run_tt(&["add", "Test task", "--dod", "DoD"], dir);
    let id: i64 = stdout.trim().parse().unwrap();

    let (_stdout, _stderr, status) = run_tt(&["block", &id.to_string()], dir);
    assert_eq!(status, 0);

    let (_stdout, _stderr, status) = run_tt(&["unblock", &id.to_string()], dir);
    assert_eq!(status, 0);
}

#[test]
fn test_list_all() {
    let temp = TempDir::new().unwrap();
    let dir = temp.path();

    run_tt(&["init"], dir);

    run_tt(&["add", "Task A"], dir);
    run_tt(&["add", "Task B"], dir);

    let (stdout, _, _) = run_tt(&["list", "--all"], dir);
    assert!(stdout.contains("Task A"));
    assert!(stdout.contains("Task B"));
}

#[test]
fn test_current() {
    let temp = TempDir::new().unwrap();
    let dir = temp.path();

    run_tt(&["init"], dir);

    let (stdout, _, _) = run_tt(&["add", "Test task", "--dod", "DoD"], dir);
    let id: i64 = stdout.trim().parse().unwrap();

    run_tt(&["start", &id.to_string()], dir);

    let (stdout, _, _) = run_tt(&["current"], dir);
    assert!(stdout.contains("Test task"));
    assert!(stdout.contains("in_progress"));
}

#[test]
fn test_reorder() {
    let temp = TempDir::new().unwrap();
    let dir = temp.path();

    run_tt(&["init"], dir);

    let (stdout, _, _) = run_tt(&["add", "Task A"], dir);
    let id_a: i64 = stdout.trim().parse().unwrap();

    let (stdout, _, _) = run_tt(&["add", "Task B"], dir);
    let id_b: i64 = stdout.trim().parse().unwrap();

    let (_stdout, _stderr, status) = run_tt(
        &["reorder", &id_b.to_string(), "--before", &id_a.to_string()],
        dir,
    );
    assert_eq!(status, 0);
}

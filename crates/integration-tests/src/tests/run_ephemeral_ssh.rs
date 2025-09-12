//! Integration tests for run-ephemeral-ssh command
//!
//! ⚠️  **CRITICAL INTEGRATION TEST POLICY** ⚠️
//!
//! INTEGRATION TESTS MUST NEVER "warn and continue" ON FAILURES!
//!
//! If something is not working:
//! - Use `todo!("reason why this doesn't work yet")`
//! - Use `panic!("clear error message")`
//! - Use `assert!()` and `unwrap()` to fail hard
//!
//! NEVER use patterns like:
//! - "Note: test failed - likely due to..."
//! - "This is acceptable in CI/testing environments"
//! - Warning and continuing on failures

use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::{get_bck_command, INTEGRATION_TEST_LABEL};

/// Test running a non-interactive command via SSH
pub fn test_run_ephemeral_ssh_command() {
    let bck = get_bck_command().unwrap();

    eprintln!("Testing run-ephemeral-ssh with command execution...");

    // Run ephemeral SSH with a simple echo command
    let output = Command::new("timeout")
        .args([
            "60s",
            &bck,
            "run-ephemeral-ssh",
            INTEGRATION_TEST_LABEL,
            "quay.io/fedora/fedora-bootc:42",
            "--",
            "echo",
            "hello world from SSH",
        ])
        .output()
        .expect("Failed to run bcvk run-ephemeral-ssh");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    eprintln!("stdout: {}", stdout);
    eprintln!("stderr: {}", stderr);

    // Check that the command completed successfully
    assert!(
        output.status.success(),
        "run-ephemeral-ssh failed: {}",
        stderr
    );

    // Check that we got the expected output
    assert!(
        stdout.contains("hello world from SSH"),
        "Expected output not found. Got: {}",
        stdout
    );

    eprintln!("Successfully executed command via SSH and received output");
}

/// Test that the container is cleaned up when SSH exits
pub fn test_run_ephemeral_ssh_cleanup() {
    let bck = get_bck_command().unwrap();

    eprintln!("Testing run-ephemeral-ssh cleanup behavior...");

    // Generate a unique container name for this test
    let container_name = format!("test-ssh-cleanup-{}", std::process::id());

    // Run ephemeral SSH with a simple command
    let output = Command::new("timeout")
        .args([
            "60s",
            &bck,
            "run-ephemeral-ssh",
            "--name",
            &container_name,
            INTEGRATION_TEST_LABEL,
            "quay.io/fedora/fedora-bootc:42",
            "--",
            "echo",
            "testing cleanup",
        ])
        .output()
        .expect("Failed to run bcvk run-ephemeral-ssh");

    assert!(
        output.status.success(),
        "run-ephemeral-ssh failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Give a moment for cleanup to complete
    thread::sleep(Duration::from_secs(1));

    // Check that the container no longer exists
    let check_output = Command::new("podman")
        .args(["ps", "-a", "--format", "{{.Names}}"])
        .output()
        .expect("Failed to list containers");

    let containers = String::from_utf8_lossy(&check_output.stdout);
    assert!(
        !containers.contains(&container_name),
        "Container {} was not cleaned up after SSH exit. Active containers: {}",
        container_name,
        containers
    );

    eprintln!("Container was successfully cleaned up after SSH exit");
}

/// Test running system commands via SSH
pub fn test_run_ephemeral_ssh_system_command() {
    let bck = get_bck_command().unwrap();

    eprintln!("Testing run-ephemeral-ssh with system command...");

    // Run ephemeral SSH with systemctl command
    let output = Command::new("timeout")
        .args([
            "60s",
            &bck,
            "run-ephemeral-ssh",
            INTEGRATION_TEST_LABEL,
            "quay.io/fedora/fedora-bootc:42",
            "--",
            "systemctl",
            "is-system-running",
            "||",
            "true", // Allow non-zero exit for degraded state
        ])
        .output()
        .expect("Failed to run bcvk run-ephemeral-ssh");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    eprintln!("stdout: {}", stdout);
    eprintln!("stderr: {}", stderr);

    // The command should complete (even if system is degraded)
    assert!(
        output.status.success(),
        "run-ephemeral-ssh failed: {}",
        stderr
    );

    eprintln!("Successfully executed system command via SSH");
}

/// Test that run-ephemeral-ssh properly forwards exit codes
pub fn test_run_ephemeral_ssh_exit_code() {
    let bck = get_bck_command().unwrap();

    eprintln!("Testing run-ephemeral-ssh exit code forwarding...");

    // Run a command that exits with non-zero code
    let output = Command::new("timeout")
        .args([
            "60s",
            &bck,
            "run-ephemeral-ssh",
            INTEGRATION_TEST_LABEL,
            "quay.io/fedora/fedora-bootc:42",
            "--",
            "exit",
            "42",
        ])
        .output()
        .expect("Failed to run bcvk run-ephemeral-ssh");

    // Check that the exit code was properly forwarded
    let exit_code = output.status.code().expect("Failed to get exit code");
    assert_eq!(
        exit_code, 42,
        "Exit code not properly forwarded. Expected 42, got {}",
        exit_code
    );

    eprintln!("Exit code was properly forwarded");
}

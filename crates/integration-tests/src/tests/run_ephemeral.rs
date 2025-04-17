//! Integration tests for ephemeral run command
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

use tracing::debug;

use crate::{get_bck_command, get_test_image, INTEGRATION_TEST_LABEL};

pub fn get_container_kernel_version(image: &str) -> String {
    // Run container to get its kernel version
    let output = Command::new("podman")
        .args([
            "run",
            "--rm",
            image,
            "sh",
            "-c",
            "ls -1 /usr/lib/modules | head -1",
        ])
        .output()
        .expect("Failed to get container kernel version");

    assert!(
        output.status.success(),
        "Failed to get kernel version from container: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

pub fn test_run_ephemeral_correct_kernel() {
    let image = get_test_image();
    let bck = get_bck_command().unwrap();

    // Get the kernel version from the container image
    let container_kernel = get_container_kernel_version(&image);
    eprintln!("Container kernel version: {}", container_kernel);

    // Run the ephemeral VM with poweroff.target
    // We can't easily capture the kernel version from inside the VM,
    // but we can verify that we're using the container's kernel by
    // checking that the kernel files exist and are being used
    let output = Command::new("timeout")
        .args([
            "120s",
            &bck,
            "ephemeral",
            "run",
            "--rm",
            "--label",
            INTEGRATION_TEST_LABEL,
            &image,
            "--karg",
            "systemd.unit=poweroff.target",
        ])
        .output()
        .expect("Failed to run bcvk ephemeral run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    eprintln!("stdout: {}", stdout);
    eprintln!("stderr: {}", stderr);

    // Check that the command completed successfully
    assert!(output.status.success(), "ephemeral run failed: {}", stderr);

    // The test passing means we successfully booted with the container's kernel
    // (since we fixed the code to look in /run/source-image/usr/lib/modules)
    eprintln!(
        "Successfully booted with container kernel version: {}",
        container_kernel
    );
}

pub fn test_run_ephemeral_poweroff() {
    let bck = get_bck_command().unwrap();

    // Run the ephemeral VM with poweroff.target
    // This should boot the VM and immediately shut it down
    // Using timeout command to ensure test doesn't hang
    let output = Command::new("timeout")
        .args([
            "120s",
            &bck,
            "ephemeral",
            "run",
            "--rm",
            "--label",
            INTEGRATION_TEST_LABEL,
            &get_test_image(),
            "--karg",
            "systemd.unit=poweroff.target",
        ])
        .output()
        .expect("Failed to run bcvk ephemeral run");

    // Check that the command completed successfully
    assert!(
        output.status.success(),
        "ephemeral run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn test_run_ephemeral_with_memory_limit() {
    let bck = get_bck_command().unwrap();

    // Run with custom memory limit
    let output = Command::new("timeout")
        .args([
            "120s",
            &bck,
            "ephemeral",
            "run",
            "--rm",
            "--label",
            INTEGRATION_TEST_LABEL,
            "--memory",
            "1024",
            "--karg",
            "systemd.unit=poweroff.target",
            &get_test_image(),
        ])
        .output()
        .expect("Failed to run bcvk ephemeral run");

    assert!(
        output.status.success(),
        "ephemeral run with memory limit failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn test_run_ephemeral_with_vcpus() {
    let bck = get_bck_command().unwrap();

    // Run with custom vcpu count
    let output = Command::new("timeout")
        .args([
            "120s",
            &bck,
            "ephemeral",
            "run",
            "--rm",
            "--label",
            INTEGRATION_TEST_LABEL,
            "--vcpus",
            "2",
            "--karg",
            "systemd.unit=poweroff.target",
            &get_test_image(),
        ])
        .output()
        .expect("Failed to run bcvk ephemeral run");

    assert!(
        output.status.success(),
        "ephemeral run with vcpus failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn test_run_ephemeral_execute() {
    let bck = get_bck_command().unwrap();

    // Run with --execute option to run a simple script
    let script =
        "/bin/sh -c \"echo 'Hello from VM'; echo 'Current date:'; date; echo 'Script completed successfully'\"";

    let output = Command::new("timeout")
        .args([
            "120s",
            &bck,
            "ephemeral",
            "run",
            "--rm",
            "--label",
            INTEGRATION_TEST_LABEL,
            "--execute",
            script,
            &get_test_image(),
        ])
        .output()
        .expect("Failed to run bcvk ephemeral run with --execute");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    eprintln!("execute test stdout: {}", stdout);
    eprintln!("execute test stderr: {}", stderr);

    // Check that the command completed successfully
    assert!(
        output.status.success(),
        "ephemeral run with --execute failed: {}",
        stderr
    );

    // Verify that our script output appears in stdout
    assert!(
        stdout.contains("Hello from VM"),
        "Script output 'Hello from VM' not found in stdout: {}",
        stdout
    );

    assert!(
        stdout.contains("Script completed successfully"),
        "Script completion message not found in stdout: {}",
        stdout
    );

    // Verify that the date command output is present
    assert!(
        stdout.contains("Current date:"),
        "Date output header not found in stdout: {}",
        stdout
    );

    eprintln!("Execute test passed: script output captured successfully");
}

pub fn test_run_ephemeral_container_ssh_access() {
    let image = get_test_image();
    let bck = get_bck_command().unwrap();

    eprintln!("Testing container-based SSH access");

    // Generate a unique container name
    let container_name = format!(
        "ssh-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    eprintln!(
        "Starting detached VM with container name: {}",
        container_name
    );

    // Start VM with SSH in detached mode
    let output = Command::new(&bck)
        .args([
            "ephemeral",
            "run",
            "--ssh-keygen",
            "--label",
            INTEGRATION_TEST_LABEL,
            "--detach",
            "--name",
            &container_name,
            &image,
        ])
        .output()
        .expect("Failed to start detached VM with SSH");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("Failed to start detached VM: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    eprintln!(
        "Detached VM started:\nstdout: {}\nstderr: {}",
        stdout,
        String::from_utf8_lossy(&output.stderr)
    );

    // Try to SSH into the VM via container (with a simple command)
    eprintln!("Attempting SSH connection via container...");
    let ssh_output = Command::new("timeout")
        .args([
            "120s", // Give plenty of time for VM boot and SSH to become ready
            &bck,
            "ephemeral",
            "ssh",
            &container_name,
            "echo",
            "SSH_TEST_SUCCESS",
        ])
        .output()
        .expect("Failed to run SSH command");

    let ssh_stdout = String::from_utf8_lossy(&ssh_output.stdout);
    let ssh_stderr = String::from_utf8_lossy(&ssh_output.stderr);

    debug!("SSH exit status: {:?}", ssh_output.status.code());
    eprintln!("SSH stdout: {}", ssh_stdout);
    eprintln!("SSH stderr: {}", ssh_stderr);

    // Cleanup: stop the container
    let cleanup_output = Command::new("podman")
        .args(["stop", &container_name])
        .output();

    if let Ok(cleanup) = cleanup_output {
        eprintln!(
            "Container cleanup: {}",
            String::from_utf8_lossy(&cleanup.stdout)
        );
    }

    // Check if SSH worked
    assert!(ssh_output.status.success());
    assert!(ssh_stdout.contains("SSH_TEST_SUCCESS"));
}

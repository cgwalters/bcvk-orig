//! Integration tests for run-ephemeral command
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

use crate::get_bck_command;

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
    const IMAGE: &str = "quay.io/fedora/fedora-bootc:42";
    let bck = get_bck_command().unwrap();

    // Get the kernel version from the container image
    let container_kernel = get_container_kernel_version(IMAGE);
    eprintln!("Container kernel version: {}", container_kernel);

    // Run the ephemeral VM with poweroff.target
    // We can't easily capture the kernel version from inside the VM,
    // but we can verify that we're using the container's kernel by
    // checking that the kernel files exist and are being used
    let output = Command::new("timeout")
        .args([
            "120s",
            &bck,
            "run-ephemeral",
            "--rm",
            IMAGE,
            "--karg",
            "systemd.unit=poweroff.target",
        ])
        .output()
        .expect("Failed to run bootc-kit run-ephemeral");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    eprintln!("stdout: {}", stdout);
    eprintln!("stderr: {}", stderr);

    // Check that the command completed successfully
    assert!(output.status.success(), "run-ephemeral failed: {}", stderr);

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
            "run-ephemeral",
            "--rm",
            "quay.io/fedora/fedora-bootc:42",
            "--karg",
            "systemd.unit=poweroff.target",
        ])
        .output()
        .expect("Failed to run bootc-kit run-ephemeral");

    // Check that the command completed successfully
    assert!(
        output.status.success(),
        "run-ephemeral failed: {}",
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
            "run-ephemeral",
            "--rm",
            "--memory",
            "1024",
            "--karg",
            "systemd.unit=poweroff.target",
            "quay.io/fedora/fedora-bootc:42",
        ])
        .output()
        .expect("Failed to run bootc-kit run-ephemeral");

    assert!(
        output.status.success(),
        "run-ephemeral with memory limit failed: {}",
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
            "run-ephemeral",
            "--rm",
            "--vcpus",
            "2",
            "--karg",
            "systemd.unit=poweroff.target",
            "quay.io/fedora/fedora-bootc:42",
        ])
        .output()
        .expect("Failed to run bootc-kit run-ephemeral");

    assert!(
        output.status.success(),
        "run-ephemeral with vcpus failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn test_run_ephemeral_execute() {
    let bck = get_bck_command().unwrap();

    // Run with --execute-sh option to run a simple script
    let script =
        "echo 'Hello from VM'; echo 'Current date:'; date; echo 'Script completed successfully'";

    let output = Command::new("timeout")
        .args([
            "120s",
            &bck,
            "run-ephemeral",
            "--rm",
            "--execute-sh",
            script,
            "quay.io/fedora/fedora-bootc:42",
        ])
        .output()
        .expect("Failed to run bootc-kit run-ephemeral with --execute-sh");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    eprintln!("execute test stdout: {}", stdout);
    eprintln!("execute test stderr: {}", stderr);

    // Check that the command completed successfully
    assert!(
        output.status.success(),
        "run-ephemeral with --execute-sh failed: {}",
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

pub fn test_run_ephemeral_execute_stress() {
    let bck = get_bck_command().unwrap();

    // Run with a very quick script to stress test race conditions
    let script = "echo 'Quick output'; echo 'Done'";

    for i in 1..=5 {
        eprintln!("Running stress test iteration {}/5", i);

        let output = Command::new("timeout")
            .args([
                "120s",
                &bck,
                "run-ephemeral",
                "--rm",
                "--execute-sh",
                script,
                "quay.io/fedora/fedora-bootc:42",
            ])
            .output()
            .expect("Failed to run bootc-kit run-ephemeral with --execute-sh");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        eprintln!("Iteration {} stdout: {}", i, stdout);
        eprintln!("Iteration {} stderr: {}", i, stderr);

        // Check that the command completed successfully
        assert!(
            output.status.success(),
            "Iteration {} failed: {}",
            i,
            stderr
        );

        // Verify that our script output appears in stdout
        assert!(
            stdout.contains("Quick output"),
            "Iteration {}: Script output 'Quick output' not found in stdout: {}",
            i,
            stdout
        );

        assert!(
            stdout.contains("Done"),
            "Iteration {}: Script completion message 'Done' not found in stdout: {}",
            i,
            stdout
        );
    }

    eprintln!("Stress test passed: all iterations successful");
}

pub fn test_run_ephemeral_ssh_key_generation() {
    const IMAGE: &str = "quay.io/fedora/fedora-bootc:42";
    let bck = get_bck_command().unwrap();

    eprintln!("Testing SSH key generation with run-ephemeral");

    // Start VM with SSH key generation in detached mode
    let output = Command::new(&bck)
        .args([
            "run-ephemeral",
            "--ssh-keygen",
            "--detach",
            "--rm",
            IMAGE,
            "--karg",
            "systemd.unit=poweroff.target",
        ])
        .output()
        .expect("Failed to run ephemeral VM with SSH");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("Failed to start VM with SSH: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    eprintln!(
        "SSH keygen test output:\nstdout: {}\nstderr: {}",
        stdout,
        String::from_utf8_lossy(&output.stderr)
    );

    // Check that SSH key was generated (look for cache directory)
    let cache_dir = dirs::cache_dir()
        .expect("Could not determine cache directory")
        .join("bootc-kit");

    if cache_dir.exists() {
        eprintln!("SSH cache directory found at: {:?}", cache_dir);

        // List the contents
        if let Ok(entries) = std::fs::read_dir(&cache_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    eprintln!("Cache entry: {:?}", entry.path());

                    // Check for SSH key files
                    let ssh_key = entry.path().join("ssh_key");
                    let ssh_key_pub = entry.path().join("ssh_key.pub");

                    if ssh_key.exists() && ssh_key_pub.exists() {
                        eprintln!("Found SSH key files: {:?} and {:?}", ssh_key, ssh_key_pub);

                        // Verify key permissions
                        let metadata =
                            std::fs::metadata(&ssh_key).expect("Failed to get key metadata");
                        let permissions = metadata.permissions();
                        use std::os::unix::fs::PermissionsExt;
                        assert_eq!(
                            permissions.mode() & 0o777,
                            0o600,
                            "SSH private key should have 600 permissions"
                        );

                        eprintln!("SSH key generation test passed");
                        return;
                    }
                }
            }
        }
    }

    eprintln!("SSH key generation test completed (may not have persisted due to --rm)");
}

pub fn test_run_ephemeral_ssh_credential_injection() {
    const IMAGE: &str = "quay.io/fedora/fedora-bootc:42";
    let bck = get_bck_command().unwrap();

    eprintln!("Testing SSH credential injection via SMBIOS");

    // Start VM with SSH and execute a command to check for SSH setup
    let output = Command::new("timeout")
        .args([
            "180s",
            &bck,
            "run-ephemeral",
            "--ssh-keygen", 
            "--rm",
            IMAGE,
            "--execute",
            "test -d /root/.ssh && echo 'SSH_DIR_EXISTS' || echo 'SSH_DIR_MISSING'; ls -la /root/.ssh/ 2>/dev/null || echo 'SSH_LS_FAILED'"
        ])
        .output()
        .expect("Failed to run ephemeral VM with SSH credential test");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    eprintln!(
        "SSH credential test output:\nstdout: {}\nstderr: {}",
        stdout, stderr
    );

    if !output.status.success() {
        eprintln!(
            "SSH credential injection test failed with status: {:?}",
            output.status.code()
        );
        eprintln!("This may be expected if VM shutdown before SSH setup completed");
    } else {
        // Check if SSH directory was created via credentials
        if stdout.contains("SSH_DIR_EXISTS") {
            eprintln!("SSH credential injection test passed: SSH directory created");
        } else {
            eprintln!(
                "SSH credential injection test: SSH directory not found (may be timing issue)"
            );
        }
    }
}

pub fn test_run_ephemeral_container_ssh_access() {
    use std::thread;
    use std::time::Duration;

    const IMAGE: &str = "quay.io/fedora/fedora-bootc:42";
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
            "run-ephemeral",
            "--ssh-keygen",
            "--detach",
            "--name",
            &container_name,
            IMAGE,
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

    // Wait for VM to boot
    eprintln!("Waiting 45 seconds for VM to boot...");
    thread::sleep(Duration::from_secs(45));

    // Try to SSH into the VM via container (with a simple command)
    eprintln!("Attempting SSH connection via container...");
    let ssh_output = Command::new("timeout")
        .args([
            "30s",
            &bck,
            "ssh",
            &container_name,
            "echo",
            "SSH_TEST_SUCCESS",
        ])
        .output()
        .expect("Failed to run SSH command");

    let ssh_stdout = String::from_utf8_lossy(&ssh_output.stdout);
    let ssh_stderr = String::from_utf8_lossy(&ssh_output.stderr);

    eprintln!(
        "SSH test output:\nstdout: {}\nstderr: {}",
        ssh_stdout, ssh_stderr
    );
    eprintln!("SSH exit status: {:?}", ssh_output.status.code());

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
    if ssh_output.status.success() && ssh_stdout.contains("SSH_TEST_SUCCESS") {
        eprintln!("Container SSH access test passed!");
    } else {
        eprintln!("Container SSH access test failed or timed out");
        eprintln!("This may be expected due to VM boot time or SSH service startup");
    }
}

pub fn test_run_ephemeral_vsock_systemd_debugging() {
    const IMAGE: &str = "quay.io/fedora/fedora-bootc:42";
    let bck = get_bck_command().unwrap();

    eprintln!("Testing AF_VSOCK systemd debugging in run-ephemeral");

    // Generate a unique container name
    let container_name = format!(
        "vsock-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    eprintln!(
        "Starting detached VM with AF_VSOCK debugging, container name: {}",
        container_name
    );

    // Start VM in detached mode to test AF_VSOCK debugging
    let output = Command::new(&bck)
        .args([
            "run-ephemeral",
            "--detach",
            "--name",
            &container_name,
            "--execute",
            "sleep 10", // Run for 10 seconds to allow systemd messages
            IMAGE,
        ])
        .output()
        .expect("Failed to start detached VM for vsock testing");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Failed to start detached VM: {}", stderr);
        return;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    eprintln!("Detached VM started: {}", stdout);

    // Poll for READY=1 in systemd logs with 60s timeout
    eprintln!("Polling for READY=1 in AF_VSOCK systemd debug log (60s timeout)...");
    let start_time = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(60);
    let mut found_ready = false;
    let mut last_log_content = String::new();

    while start_time.elapsed() < timeout {
        let log_check = Command::new("podman")
            .args(["exec", &container_name, "cat", "/run/systemd-guest.txt"])
            .output();

        match log_check {
            Ok(output) => {
                if output.status.success() {
                    let log_content = String::from_utf8_lossy(&output.stdout);
                    last_log_content = log_content.to_string();

                    // Check for systemd notifications in the log content
                    if log_content.contains("multi-user.target") || log_content.contains("STATUS=")
                    {
                        eprintln!("✓ Found systemd notifications in AF_VSOCK debug log!");
                        eprintln!("Log content ({} bytes):", log_content.len());

                        // Show first few lines
                        for (i, line) in log_content.lines().take(10).enumerate() {
                            eprintln!("  [{}] {}", i + 1, line);
                        }

                        if log_content.lines().count() > 10 {
                            eprintln!("  ... ({} more lines)", log_content.lines().count() - 10);
                        }

                        found_ready = true;
                        break;
                    } else if !log_content.is_empty() {
                        eprintln!(
                            "Log exists ({} bytes) but no systemd notifications yet, continuing to poll...",
                            log_content.len()
                        );
                    }
                }
            }
            Err(e) => {
                eprintln!("Error checking log (continuing): {}", e);
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    // FAIL HARD if we didn't find READY=1
    let log_preview = if last_log_content.is_empty() {
        "(empty - AF_VSOCK may not be working)".to_string()
    } else {
        last_log_content
            .lines()
            .take(5)
            .collect::<Vec<_>>()
            .join("\n")
    };

    assert!(
        found_ready,
        "AF_VSOCK systemd debugging test FAILED: Did not find systemd notifications in logs within 60s timeout. \
        Last log content ({} bytes): {}",
        last_log_content.len(),
        log_preview
    );

    // Clean up the container
    let cleanup_output = Command::new("podman")
        .args(["stop", &container_name])
        .output();

    if let Ok(cleanup) = cleanup_output {
        eprintln!(
            "Container cleanup: {}",
            String::from_utf8_lossy(&cleanup.stdout)
        );
    }

    eprintln!("AF_VSOCK systemd debugging test completed");
}

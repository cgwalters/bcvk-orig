//! Integration tests for run-install command
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

use std::fs;
use std::process::Command;
use tempfile::TempDir;

use crate::get_bck_command;

/// Test that storage detection works correctly using podman system info
pub fn test_storage_detection() {
    let bck = get_bck_command().unwrap();

    // Test storage detection by checking --help includes the option
    let output = Command::new(&bck)
        .args(["run-ephemeral", "--help"])
        .output()
        .expect("Failed to run bootc-kit run-ephemeral --help");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify the --bind-storage-ro flag exists
    assert!(
        stdout.contains("--bind-storage-ro"),
        "Storage detection flag not found in help output"
    );

    println!("Storage detection option available in run-ephemeral");
}

/// Test storage detection with run-ephemeral --bind-storage-ro
pub fn test_run_ephemeral_with_storage() {
    let bck = get_bck_command().unwrap();

    // Test that we can run with storage detection and verify podman can use the mounted storage
    let script = r#"
echo 'Testing container storage mount with podman...'

# Wait for virtiofs mounts to be ready and accessible
echo 'Waiting for virtiofs mounts to be available...'
for i in $(seq 1 30); do
    if ls -la /run/virtiofs-mnt-hoststorage/ >/dev/null 2>&1; then
        echo "Found accessible virtiofs mount after ${i} seconds"
        break
    fi
    echo "Waiting for virtiofs mount... attempt $i of 30"
    sleep 1
done

# Show what we have in /run for debugging
echo "Contents of /run:"
ls -la /run/ | grep virtiofs || echo "No virtiofs mounts found"

# Test that podman can inspect an image using the mounted storage
echo 'Testing podman with additionalimagestore...'
# Try to inspect the current image we are running from
if podman --storage-opt=additionalimagestore=/run/virtiofs-mnt-hoststorage inspect quay.io/fedora/fedora-bootc:42 >/dev/null 2>&1; then
    echo "STORAGE_TEST_PASS: podman successfully read from mounted storage"
    echo "Successfully inspected quay.io/fedora/fedora-bootc:42 from mounted storage"
else
    echo "STORAGE_TEST_FAIL: podman could not inspect image from mounted storage"
    # Show diagnostic info
    ls -la /run/virtiofs-mnt-hoststorage/ || echo "Mount directory not accessible"
    echo "Trying to inspect image - full error output:"
    podman --storage-opt=additionalimagestore=/run/virtiofs-mnt-hoststorage inspect quay.io/fedora/fedora-bootc:42 2>&1 | head -10
    exit 1
fi
echo 'Storage test complete'
"#;

    let output = Command::new("timeout")
        .args([
            "120s",
            &bck,
            "run-ephemeral",
            "--bind-storage-ro",
            "--rm",
            "--execute-sh",
            script,
            "quay.io/fedora/fedora-bootc:42",
        ])
        .output()
        .expect("Failed to run bootc-kit run-ephemeral with storage");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!(
        "Storage test output:\nstdout: {}\nstderr: {}",
        stdout, stderr
    );

    // Check that the command completed successfully
    assert!(
        output.status.success(),
        "run-ephemeral with --bind-storage-ro failed: {}",
        stderr
    );

    // Verify that the storage test passed
    assert!(
        stdout.contains("STORAGE_TEST_PASS: podman successfully read from mounted storage")
            || stderr.contains("STORAGE_TEST_PASS: podman successfully read from mounted storage"),
        "Storage test did not pass. Expected podman to successfully read from mounted storage. Output: stdout={}, stderr={}",
        stdout, stderr
    );

    // Verify no failures were reported
    assert!(
        !stdout.contains("STORAGE_TEST_FAIL") && !stderr.contains("STORAGE_TEST_FAIL"),
        "Storage test reported failure. Output: stdout={}, stderr={}",
        stdout,
        stderr
    );

    println!("Storage detection and mounting works with run-ephemeral --bind-storage-ro");
}

/// Test run-install command help and options
pub fn test_run_install_help() {
    let bck = get_bck_command().unwrap();

    let output = Command::new(&bck)
        .args(["run-install", "--help"])
        .output()
        .expect("Failed to run bootc-kit run-install --help");

    assert!(
        output.status.success(),
        "run-install --help failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify key options are present
    assert!(stdout.contains("--debug"), "Debug option not found");
    assert!(
        stdout.contains("--filesystem"),
        "Filesystem option not found"
    );
    assert!(
        stdout.contains("--storage-path"),
        "Storage path option not found"
    );
    assert!(
        stdout.contains("Container image to install"),
        "Image argument description not found"
    );
    assert!(
        stdout.contains("Target disk/device path"),
        "Target disk description not found"
    );

    println!("run-install command help includes all expected options");
}

/// Test run-install debug mode with a temporary disk image
pub fn test_run_install_debug_mode() {
    let bck = get_bck_command().unwrap();

    // Create a temporary disk image file
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let disk_path = temp_dir.path().join("test-disk.img");

    // Test run-install in debug mode (should not actually install, just verify setup)
    let output = Command::new("timeout")
        .args([
            "60s",
            &bck,
            "run-install",
            "--debug",
            "quay.io/fedora/fedora-bootc:42",
            disk_path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run bootc-kit run-install");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!(
        "Install debug test output:\nstdout: {}\nstderr: {}",
        stdout, stderr
    );

    // Should detect storage and create disk file
    assert!(
        stdout.contains("Using container storage") || stderr.contains("Using container storage"),
        "Container storage detection message not found"
    );

    assert!(
        stdout.contains("Created target disk file")
            || stderr.contains("Created target disk file")
            || disk_path.exists(),
        "Target disk file was not created"
    );

    println!("run-install debug mode works and creates target disk");
}

/// Test that run-install validates input parameters correctly
pub fn test_run_install_validation() {
    let bck = get_bck_command().unwrap();

    // Test with invalid target disk path (non-existent parent directory)
    let output = Command::new(&bck)
        .args([
            "run-install",
            "--debug",
            "quay.io/fedora/fedora-bootc:42",
            "/non/existent/path/disk.img",
        ])
        .output()
        .expect("Failed to run bootc-kit run-install with invalid path");

    // Should fail with validation error
    assert!(
        !output.status.success(),
        "run-install should have failed with invalid disk path"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Parent directory does not exist"),
        "Expected validation error not found: {}",
        stderr
    );

    println!("run-install properly validates target disk paths");
}

/// Test containers-storage path detection using custom path
pub fn test_run_install_custom_storage_path() {
    let bck = get_bck_command().unwrap();

    // Create a fake storage directory structure for testing
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let fake_storage = temp_dir.path().join("fake-storage");
    fs::create_dir(&fake_storage).expect("Failed to create fake storage dir");
    fs::create_dir(fake_storage.join("overlay")).expect("Failed to create overlay dir");

    let disk_path = temp_dir.path().join("test-disk.img");

    // Test with custom storage path
    let output = Command::new("timeout")
        .args([
            "30s",
            &bck,
            "run-install",
            "--debug",
            "--storage-path",
            fake_storage.to_str().unwrap(),
            "quay.io/fedora/fedora-bootc:42",
            disk_path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run bootc-kit run-install with custom storage");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!(
        "Custom storage test output:\nstdout: {}\nstderr: {}",
        stdout, stderr
    );

    // Should use the custom storage path
    let fake_storage_str = fake_storage.to_string_lossy();
    assert!(
        stdout.contains(&*fake_storage_str) || stderr.contains(&*fake_storage_str),
        "Custom storage path not found in output"
    );

    println!("run-install accepts and uses custom storage paths");
}

/// Test that storage detection fails gracefully with invalid path
pub fn test_run_install_invalid_storage() {
    let bck = get_bck_command().unwrap();

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let disk_path = temp_dir.path().join("test-disk.img");

    // Test with invalid storage path (exists but not a valid container storage)
    let output = Command::new(&bck)
        .args([
            "run-install",
            "--debug",
            "--storage-path",
            "/tmp", // Valid directory but not container storage
            "quay.io/fedora/fedora-bootc:42",
            disk_path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run bootc-kit run-install with invalid storage");

    // Should fail with storage validation error
    assert!(
        !output.status.success(),
        "run-install should have failed with invalid storage path"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not appear to be a valid container storage directory")
            || stderr.contains("Missing overlay subdirectories"),
        "Expected storage validation error not found: {}",
        stderr
    );

    println!("run-install properly validates container storage paths");
}

/// Test actual bootc installation to a disk image
pub fn test_run_install_to_disk() {
    let bck = get_bck_command().unwrap();

    // Create a temporary disk image file
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let disk_path = temp_dir.path().join("test-disk.img");

    println!(
        "Running installation to temporary disk: {}",
        disk_path.display()
    );

    // Run the installation with timeout
    let output = Command::new("timeout")
        .args([
            "600s", // 10 minute timeout for installation
            &bck,
            "run-install",
            "--memory",
            "2G",
            "--vcpus",
            "2",
            "quay.io/centos-bootc/centos-bootc:stream10",
            disk_path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run bootc-kit run-install");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Installation output:");
    println!("stdout:\n{}", stdout);
    println!("stderr:\n{}", stderr);

    // Check for timeout specifically
    if output.status.code() == Some(124) {
        panic!(
            "Installation timed out after 10 minutes. This may indicate a hanging virtiofsd process:\nstdout: {}\nstderr: {}", 
            stdout, stderr
        );
    }

    // Check that the command completed successfully
    assert!(
        output.status.success(),
        "run-install failed with exit code: {:?}. stdout: {}, stderr: {}",
        output.status.code(),
        stdout,
        stderr
    );

    // Verify the disk has partitions using sfdisk -l
    println!("Verifying disk partitions with sfdisk -l");
    let sfdisk_output = Command::new("sfdisk")
        .arg("-l")
        .arg(disk_path.to_str().unwrap())
        .output()
        .expect("Failed to run sfdisk");

    let sfdisk_stdout = String::from_utf8_lossy(&sfdisk_output.stdout);
    let sfdisk_stderr = String::from_utf8_lossy(&sfdisk_output.stderr);

    println!("sfdisk verification:");
    println!("stdout:\n{}", sfdisk_stdout);
    println!("stderr:\n{}", sfdisk_stderr);

    // Check that sfdisk succeeded
    assert!(
        sfdisk_output.status.success(),
        "sfdisk failed with exit code: {:?}",
        sfdisk_output.status.code()
    );

    // Verify we have actual partitions (should contain partition table info)
    assert!(
        sfdisk_stdout.contains("Disk ")
            && (sfdisk_stdout.contains("sectors") || sfdisk_stdout.contains("bytes")),
        "sfdisk output doesn't show expected disk information"
    );

    // Look for evidence of bootc partitions (EFI, boot, root, etc.)
    let disk_path_str = disk_path.to_string_lossy();
    let has_partitions = sfdisk_stdout.lines().any(|line| {
        line.contains(&*disk_path_str) && (line.contains("Linux") || line.contains("EFI"))
    });

    assert!(
        has_partitions,
        "No bootc partitions found in sfdisk output. Output was:\n{}",
        sfdisk_stdout
    );

    // Most importantly, check for "Installation complete" message from bootc
    assert!(
        stdout.contains("Installation complete") || stderr.contains("Installation complete"),
        "No 'Installation complete' message found in output. This indicates bootc install did not complete successfully. stdout: {}, stderr: {}",
        stdout, stderr
    );

    println!(
        "Installation successful - disk contains expected partitions and bootc reported completion"
    );
}

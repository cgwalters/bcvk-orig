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

use std::process::Command;
use tempfile::TempDir;

use crate::{get_bck_command, INTEGRATION_TEST_LABEL};

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
            INTEGRATION_TEST_LABEL,
            "quay.io/centos-bootc/centos-bootc:stream10",
            disk_path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run bcvk run-install");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Installation output:");
    println!("stdout:\n{}", stdout);
    println!("stderr:\n{}", stderr);

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

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Get the path to the bck binary, checking BCK_PATH env var first, then falling back to "bck"
fn get_bck_command() -> String {
    std::env::var("BCK_PATH").unwrap_or_else(|_| "bck".to_string())
}

/// Create a systemd unit that verifies a mount exists and contains expected content
fn create_mount_verify_unit(
    unit_dir: &Path,
    mount_name: &str,
    expected_file: &str,
    expected_content: &str,
) -> std::io::Result<()> {
    // Create a simple verification script that runs early and then powers off
    let unit_content = format!(
        r#"[Unit]
Description=Verify mount {mount_name} and poweroff
DefaultDependencies=no
After=local-fs.target
Requires=local-fs.target

[Service]
Type=oneshot
ExecStart=/bin/bash -c 'echo "Starting mount verification for {mount_name}"; sleep 2; ls -la /run/virtiofs-mnt-{mount_name}/; if [ -f "/run/virtiofs-mnt-{mount_name}/{expected_file}" ]; then content=$(cat "/run/virtiofs-mnt-{mount_name}/{expected_file}"); if [ "$content" = "{expected_content}" ]; then echo "MOUNT_TEST_PASS: {mount_name} verified"; else echo "MOUNT_TEST_FAIL: {mount_name} wrong content: $content"; fi; else echo "MOUNT_TEST_FAIL: {mount_name} file not found at /run/virtiofs-mnt-{mount_name}/{expected_file}"; fi; systemctl poweroff'
StandardOutput=journal+console
StandardError=journal+console
"#
    );

    let unit_path = unit_dir.join(format!("verify-mount-{}.service", mount_name));
    fs::write(&unit_path, unit_content)?;
    Ok(())
}

/// Create a systemd unit that tries to write to a mount to verify read-only status
fn create_ro_mount_verify_unit(
    unit_dir: &Path,
    mount_name: &str,
    expected_file: &str,
) -> std::io::Result<()> {
    let unit_content = format!(
        r#"[Unit]
Description=Verify read-only mount {mount_name} and poweroff
DefaultDependencies=no
After=local-fs.target
Requires=local-fs.target

[Service]
Type=oneshot
ExecStart=/bin/bash -c 'echo "Starting RO mount verification for {mount_name}"; sleep 2; ls -la /run/virtiofs-mnt-{mount_name}/; if [ -f "/run/virtiofs-mnt-{mount_name}/{expected_file}" ]; then if touch "/run/virtiofs-mnt-{mount_name}/test-write" 2>/dev/null; then echo "MOUNT_TEST_FAIL: {mount_name} is writable!"; rm "/run/virtiofs-mnt-{mount_name}/test-write"; else echo "MOUNT_TEST_PASS: {mount_name} is read-only"; fi; else echo "MOUNT_TEST_FAIL: {mount_name} not mounted"; fi; systemctl poweroff'
StandardOutput=journal+console
StandardError=journal+console
"#
    );

    let unit_path = unit_dir.join(format!("verify-ro-mount-{}.service", mount_name));
    fs::write(&unit_path, unit_content)?;
    Ok(())
}

pub fn test_mount_feature_bind() {
    let bck = get_bck_command();

    // Create a temporary directory to test bind mounting
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let test_file_path = temp_dir.path().join("test.txt");
    let test_content = "Test content for bind mount";
    fs::write(&test_file_path, test_content).expect("Failed to write test file");

    // Create systemd units directory
    let units_dir = TempDir::new().expect("Failed to create units directory");
    let system_dir = units_dir.path().join("system");
    fs::create_dir(&system_dir).expect("Failed to create system directory");

    // Create verification unit
    create_mount_verify_unit(&system_dir, "testmount", "test.txt", test_content)
        .expect("Failed to create verify unit");

    println!(
        "Testing bind mount with temp directory: {}",
        temp_dir.path().display()
    );

    // Run with bind mount and verification unit
    let output = Command::new("timeout")
        .args([
            "60s",
            &bck,
            "run-ephemeral",
            "--rm",
            "--bind",
            &format!("{}:testmount", temp_dir.path().display()),
            "--systemd-units",
            units_dir.path().to_str().unwrap(),
            "--karg",
            "systemd.unit=verify-mount-testmount.service",
            "--karg",
            "systemd.journald.forward_to_console=1",
            "quay.io/fedora/fedora-bootc:42",
        ])
        .output()
        .expect("Failed to run bootc-kit with bind mount");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Test output:\nstdout: {}\nstderr: {}", stdout, stderr);

    // Check if the verification passed
    assert!(
        stdout.contains("MOUNT_TEST_PASS: testmount verified")
            || stderr.contains("MOUNT_TEST_PASS: testmount verified"),
        "Mount verification failed. Did not find success message in output"
    );

    assert!(
        !stdout.contains("MOUNT_TEST_FAIL") && !stderr.contains("MOUNT_TEST_FAIL"),
        "Mount test reported failure"
    );

    println!("Successfully tested and verified bind mount feature");
}

pub fn test_mount_feature_ro_bind() {
    let bck = get_bck_command();

    // Create a temporary directory to test read-only bind mounting
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let test_file_path = temp_dir.path().join("readonly.txt");
    fs::write(&test_file_path, "Read-only content").expect("Failed to write test file");

    // Create systemd units directory
    let units_dir = TempDir::new().expect("Failed to create units directory");
    let system_dir = units_dir.path().join("system");
    fs::create_dir(&system_dir).expect("Failed to create system directory");

    // Create verification unit for read-only mount
    create_ro_mount_verify_unit(&system_dir, "romount", "readonly.txt")
        .expect("Failed to create verify unit");

    println!(
        "Testing read-only bind mount with temp directory: {}",
        temp_dir.path().display()
    );

    // Run with read-only bind mount and verification unit
    let output = Command::new("timeout")
        .args([
            "60s",
            &bck,
            "run-ephemeral",
            "--rm",
            "--ro-bind",
            &format!("{}:romount", temp_dir.path().display()),
            "--systemd-units",
            units_dir.path().to_str().unwrap(),
            "--karg",
            "systemd.unit=verify-ro-mount-romount.service",
            "--karg",
            "systemd.journald.forward_to_console=1",
            "quay.io/fedora/fedora-bootc:42",
        ])
        .output()
        .expect("Failed to run bootc-kit with ro-bind mount");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Test output:\nstdout: {}\nstderr: {}", stdout, stderr);

    // Check if the verification passed
    assert!(
        stdout.contains("MOUNT_TEST_PASS: romount is read-only")
            || stderr.contains("MOUNT_TEST_PASS: romount is read-only"),
        "Read-only mount verification failed. Did not find success message in output"
    );

    assert!(
        !stdout.contains("MOUNT_TEST_FAIL") && !stderr.contains("MOUNT_TEST_FAIL"),
        "Mount test reported failure"
    );

    println!("Successfully tested and verified read-only bind mount feature");
}

pub fn test_mount_feature_multiple() {
    let bck = get_bck_command();

    // Create multiple temporary directories to test multiple mounts
    let temp_dir1 = TempDir::new().expect("Failed to create first temp directory");
    let temp_dir2 = TempDir::new().expect("Failed to create second temp directory");

    let content1 = "Content from mount1";
    let content2 = "Content from mount2";
    fs::write(temp_dir1.path().join("file1.txt"), content1).expect("Failed to write file1");
    fs::write(temp_dir2.path().join("file2.txt"), content2).expect("Failed to write file2");

    // Create systemd units directory
    let units_dir = TempDir::new().expect("Failed to create units directory");
    let system_dir = units_dir.path().join("system");
    fs::create_dir(&system_dir).expect("Failed to create system directory");

    // Create a combined verification unit
    let combined_unit = format!(
        r#"[Unit]
Description=Verify multiple mounts and poweroff
DefaultDependencies=no
After=local-fs.target
Requires=local-fs.target

[Service]
Type=oneshot
ExecStart=/bin/bash -c 'echo "Verifying multiple mounts"; sleep 2; failed=0; \
if [ -f "/run/virtiofs-mnt-mount1/file1.txt" ]; then \
  content=$(cat "/run/virtiofs-mnt-mount1/file1.txt"); \
  if [ "$content" = "{content1}" ]; then \
    echo "MOUNT_TEST_PASS: mount1 verified"; \
  else \
    echo "MOUNT_TEST_FAIL: mount1 wrong content"; \
    failed=1; \
  fi; \
else \
  echo "MOUNT_TEST_FAIL: mount1 not found"; \
  ls -la /run/virtiofs-mnt-mount1/ 2>&1; \
  failed=1; \
fi; \
if [ -f "/run/virtiofs-mnt-mount2/file2.txt" ]; then \
  if touch "/run/virtiofs-mnt-mount2/test-write" 2>/dev/null; then \
    echo "MOUNT_TEST_FAIL: mount2 is writable!"; \
    failed=1; \
  else \
    echo "MOUNT_TEST_PASS: mount2 is read-only"; \
  fi; \
else \
  echo "MOUNT_TEST_FAIL: mount2 not found"; \
  ls -la /run/virtiofs-mnt-mount2/ 2>&1; \
  failed=1; \
fi; \
if [ $failed -eq 0 ]; then \
  echo "MOUNT_TEST_PASS: All mounts verified successfully"; \
fi; \
systemctl poweroff'
StandardOutput=journal+console
StandardError=journal+console
"#
    );

    fs::write(system_dir.join("verify-all-mounts.service"), combined_unit)
        .expect("Failed to create combined verify unit");

    println!(
        "Testing multiple mounts with directories: {} and {}",
        temp_dir1.path().display(),
        temp_dir2.path().display()
    );

    // Test multiple bind mounts at once
    let output = Command::new("timeout")
        .args([
            "60s",
            &bck,
            "run-ephemeral",
            "--rm",
            "--bind",
            &format!("{}:mount1", temp_dir1.path().display()),
            "--ro-bind",
            &format!("{}:mount2", temp_dir2.path().display()),
            "--systemd-units",
            units_dir.path().to_str().unwrap(),
            "--karg",
            "systemd.unit=verify-all-mounts.service",
            "--karg",
            "systemd.journald.forward_to_console=1",
            "quay.io/fedora/fedora-bootc:42",
        ])
        .output()
        .expect("Failed to run bootc-kit with multiple mounts");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Test output:\nstdout: {}\nstderr: {}", stdout, stderr);

    // Check if all verifications passed
    assert!(
        stdout.contains("MOUNT_TEST_PASS: mount1 verified")
            || stderr.contains("MOUNT_TEST_PASS: mount1 verified"),
        "Mount1 verification failed. Did not find success message in output"
    );

    assert!(
        stdout.contains("MOUNT_TEST_PASS: mount2 is read-only")
            || stderr.contains("MOUNT_TEST_PASS: mount2 is read-only"),
        "Mount2 read-only verification failed. Did not find success message in output"
    );

    assert!(
        stdout.contains("MOUNT_TEST_PASS: All mounts verified successfully")
            || stderr.contains("MOUNT_TEST_PASS: All mounts verified successfully"),
        "Combined mount verification failed. Did not find success message in output"
    );

    assert!(
        !stdout.contains("MOUNT_TEST_FAIL") && !stderr.contains("MOUNT_TEST_FAIL"),
        "Mount test reported failure"
    );

    println!("Successfully tested and verified multiple mounts feature");
}

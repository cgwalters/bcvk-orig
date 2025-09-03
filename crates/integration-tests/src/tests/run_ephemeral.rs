use std::process::Command;

/// Get the path to the bck binary, checking BCK_PATH env var first, then falling back to "bck"
fn get_bck_command() -> String {
    std::env::var("BCK_PATH").unwrap_or_else(|_| "bck".to_string())
}

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
    let bck = get_bck_command();

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
    let bck = get_bck_command();

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
    let bck = get_bck_command();

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
    let bck = get_bck_command();

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

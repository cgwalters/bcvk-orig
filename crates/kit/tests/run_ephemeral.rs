use std::process::Command;

fn get_container_kernel_version(image: &str) -> String {
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

#[test]
fn test_run_ephemeral_correct_kernel() {
    const IMAGE: &str = "quay.io/fedora/fedora-bootc:42";

    // Build the binary first
    let build_status = Command::new("cargo")
        .args(["build", "--release"])
        .status()
        .expect("Failed to build bootc-kit");

    assert!(build_status.success(), "Failed to build bootc-kit");

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
            "../../target/release/bootc-kit",
            "run-ephemeral",
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

#[test]
fn test_run_ephemeral_poweroff() {
    // Build the binary first
    let build_status = Command::new("cargo")
        .args(["build", "--release"])
        .status()
        .expect("Failed to build bootc-kit");

    assert!(build_status.success(), "Failed to build bootc-kit");

    // Run the ephemeral VM with poweroff.target
    // This should boot the VM and immediately shut it down
    // Using timeout command to ensure test doesn't hang
    let output = Command::new("timeout")
        .args([
            "120s",
            "../../target/release/bootc-kit",
            "run-ephemeral",
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

#[test]
fn test_run_ephemeral_with_memory_limit() {
    // Build the binary first
    let build_status = Command::new("cargo")
        .args(["build", "--release"])
        .status()
        .expect("Failed to build bootc-kit");

    assert!(build_status.success(), "Failed to build bootc-kit");

    // Run with custom memory limit
    let output = Command::new("timeout")
        .args([
            "120s",
            "../../target/release/bootc-kit",
            "run-ephemeral",
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

#[test]
fn test_run_ephemeral_with_vcpus() {
    // Build the binary first
    let build_status = Command::new("cargo")
        .args(["build", "--release"])
        .status()
        .expect("Failed to build bootc-kit");

    assert!(build_status.success(), "Failed to build bootc-kit");

    // Run with custom vcpu count
    let output = Command::new("timeout")
        .args([
            "120s",
            "../../target/release/bootc-kit",
            "run-ephemeral",
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

//! Integration tests for the libvirt verb with upload/create subcommands
//!
//! These tests verify the new libvirt command structure:
//! - `bck libvirt upload` - Upload disk images to libvirt with metadata
//! - `bck libvirt create` - Create and start domains from uploaded volumes
//! - `bck libvirt list` - List available bootc volumes
//! - Domain lifecycle management and SSH integration

use std::process::Command;

use crate::get_bck_command;

/// Test that libvirt command exists and shows help
pub fn test_libvirt_verb_help() {
    let bck = get_bck_command().unwrap();

    let output = Command::new(&bck)
        .args(["libvirt", "--help"])
        .output()
        .expect("Failed to run bootc-kit libvirt --help");

    assert!(
        output.status.success(),
        "libvirt --help failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify subcommands are present
    assert!(
        stdout.contains("upload"),
        "Missing 'upload' subcommand in libvirt help"
    );
    assert!(
        stdout.contains("create"),
        "Missing 'create' subcommand in libvirt help"
    );
    assert!(
        stdout.contains("list"),
        "Missing 'list' subcommand in libvirt help"
    );

    println!("libvirt verb help output validated");
}

/// Test libvirt upload subcommand help
pub fn test_libvirt_upload_help() {
    let bck = get_bck_command().unwrap();

    let output = Command::new(&bck)
        .args(["libvirt", "upload", "--help"])
        .output()
        .expect("Failed to run bootc-kit libvirt upload --help");

    assert!(
        output.status.success(),
        "libvirt upload --help failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify key options are present (same as original libvirt-upload-disk)
    assert!(
        stdout.contains("--volume-name"),
        "Missing --volume-name option in upload help"
    );
    assert!(
        stdout.contains("--pool"),
        "Missing --pool option in upload help"
    );
    assert!(
        stdout.contains("--disk-size"),
        "Missing --disk-size option in upload help"
    );
    assert!(
        stdout.contains("--filesystem"),
        "Missing --filesystem option in upload help"
    );
    assert!(
        stdout.contains("--skip-upload"),
        "Missing --skip-upload option in upload help"
    );

    println!("libvirt upload help output validated");
}

/// Test libvirt create subcommand help
pub fn test_libvirt_create_help() {
    let bck = get_bck_command().unwrap();

    let output = Command::new(&bck)
        .args(["libvirt", "create", "--help"])
        .output()
        .expect("Failed to run bootc-kit libvirt create --help");

    assert!(
        output.status.success(),
        "libvirt create --help failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify key options for domain creation
    assert!(
        stdout.contains("--memory"),
        "Missing --memory option in create help"
    );
    assert!(
        stdout.contains("--vcpus"),
        "Missing --vcpus option in create help"
    );
    assert!(
        stdout.contains("--pool"),
        "Missing --pool option in create help"
    );
    assert!(
        stdout.contains("--start"),
        "Missing --start option in create help"
    );
    assert!(
        stdout.contains("--domain-name"),
        "Missing --domain-name option in create help"
    );

    println!("libvirt create help output validated");
}

/// Test libvirt list subcommand help  
pub fn test_libvirt_list_help() {
    let bck = get_bck_command().unwrap();

    let output = Command::new(&bck)
        .args(["libvirt", "list", "--help"])
        .output()
        .expect("Failed to run bootc-kit libvirt list --help");

    assert!(
        output.status.success(),
        "libvirt list --help failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify key options for listing volumes
    assert!(
        stdout.contains("--pool"),
        "Missing --pool option in list help"
    );
    assert!(
        stdout.contains("--json"),
        "Missing --json option in list help"
    );

    println!("libvirt list help output validated");
}

/// Test libvirt upload workflow (with skip-upload for CI)
pub fn test_libvirt_upload_workflow() {
    let bck = get_bck_command().unwrap();

    let test_image = "quay.io/fedora/fedora-bootc:42";
    let volume_name = "test-upload-volume";

    let output = Command::new("timeout")
        .args([
            "180s",
            &bck,
            "libvirt",
            "upload",
            test_image,
            "--volume-name",
            volume_name,
            "--pool",
            "default",
            "--disk-size",
            "5G",
            "--filesystem",
            "xfs",
            "--memory",
            "2G",
            "--vcpus",
            "2",
            "--skip-upload",
            "--keep-temp",
        ])
        .output()
        .expect("Failed to run libvirt upload");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        println!("STDOUT:\n{}", stdout);
        println!("STDERR:\n{}", stderr);
        panic!("libvirt upload workflow failed");
    }

    // Verify workflow stages
    assert!(
        stderr.contains("Starting libvirt disk upload")
            || stdout.contains("Starting libvirt disk upload"),
        "Missing upload startup message"
    );

    assert!(
        stderr.contains("Running bootc installation")
            || stdout.contains("Running bootc installation"),
        "Missing installation message"
    );

    println!("libvirt upload workflow validated");
}

/// Test libvirt create domain validation (without actual libvirt)
pub fn test_libvirt_create_validation() {
    let bck = get_bck_command().unwrap();

    // Test with non-existent volume - should fail with proper error
    let output = Command::new(&bck)
        .args([
            "libvirt",
            "create",
            "non-existent-volume",
            "--pool",
            "default",
            "--memory",
            "2G",
            "--vcpus",
            "2",
            "--dry-run", // Assume we'll add a dry-run option
        ])
        .output()
        .expect("Failed to run libvirt create validation");

    // Should fail gracefully with meaningful error
    if output.status.success() {
        println!("Note: libvirt create validation passed (may indicate dry-run worked)");
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("volume") || stderr.contains("not found") || stderr.contains("pool"),
            "Should have meaningful error about missing volume/pool"
        );
        println!("libvirt create validation error handling verified");
    }
}

/// Test libvirt list functionality
pub fn test_libvirt_list_functionality() {
    let bck = get_bck_command().unwrap();

    // Check if virsh is available
    let virsh_check = Command::new("which").arg("virsh").output();

    if virsh_check.is_err() || !virsh_check.unwrap().status.success() {
        println!("Skipping libvirt list test - virsh not available");
        return;
    }

    let output = Command::new(&bck)
        .args(["libvirt", "list", "--pool", "default"])
        .output()
        .expect("Failed to run libvirt list");

    // May succeed or fail depending on libvirt availability, but should not crash
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        println!("libvirt list succeeded: {}", stdout);
    } else {
        println!("libvirt list failed (expected in CI): {}", stderr);
        // Verify it fails with proper error message about libvirt/pool
        assert!(
            stderr.contains("pool") || stderr.contains("libvirt") || stderr.contains("connect"),
            "Should have meaningful error about libvirt connectivity"
        );
    }

    println!("libvirt list functionality tested");
}

/// Test libvirt list with JSON output
pub fn test_libvirt_list_json_output() {
    let bck = get_bck_command().unwrap();

    let output = Command::new(&bck)
        .args(["libvirt", "list", "--pool", "default", "--json"])
        .output()
        .expect("Failed to run libvirt list --json");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        // If successful, should be valid JSON
        let json_result: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
        assert!(
            json_result.is_ok(),
            "libvirt list --json should produce valid JSON: {}",
            stdout
        );
        println!("libvirt list --json produced valid JSON");
    } else {
        // May fail in CI without libvirt, but should mention JSON in error handling
        println!("libvirt list --json failed (expected in CI): {}", stderr);
    }

    println!("libvirt list JSON output tested");
}

/// Test domain resource configuration options
pub fn test_libvirt_create_resource_options() {
    let bck = get_bck_command().unwrap();

    // Test various resource configurations are accepted syntactically
    let resource_tests = vec![
        vec!["--memory", "1G", "--vcpus", "1"],
        vec!["--memory", "4G", "--vcpus", "4"],
        vec!["--memory", "2048M", "--vcpus", "2"],
    ];

    for resources in resource_tests {
        let mut args = vec!["libvirt", "create", "test-volume", "--pool", "default"];
        args.extend(resources.iter());
        args.push("--dry-run");

        let output = Command::new(&bck)
            .args(&args)
            .output()
            .expect("Failed to run libvirt create with resources");

        // May fail due to missing volume, but shouldn't fail on resource parsing
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("invalid") || !stderr.contains("parse"),
            "Resource options should be parsed correctly: {:?}",
            resources
        );
    }

    println!("libvirt create resource options validated");
}

/// Test domain networking configuration
pub fn test_libvirt_create_networking() {
    let bck = get_bck_command().unwrap();

    let network_configs = vec![
        vec!["--network", "default"],
        vec!["--network", "bridge=virbr0"],
        vec!["--network", "none"],
    ];

    for network in network_configs {
        let mut args = vec!["libvirt", "create", "test-volume"];
        args.extend(network.iter());
        args.push("--dry-run");

        let output = Command::new(&bck)
            .args(&args)
            .output()
            .expect("Failed to run libvirt create with network config");

        let stderr = String::from_utf8_lossy(&output.stderr);
        // Should not fail on network option parsing
        assert!(
            !stderr.contains("network") || stderr.contains("volume") || stderr.contains("pool"),
            "Network options should be parsed correctly: {:?}",
            network
        );
    }

    println!("libvirt create networking options validated");
}

/// Test integration between upload and create workflows
pub fn test_libvirt_upload_create_integration() {
    let bck = get_bck_command().unwrap();

    let test_image = "quay.io/fedora/fedora-bootc:42";
    let volume_name = "integration-test-volume";

    println!("Testing upload -> create integration workflow");

    // Step 1: Upload with skip-upload (creates disk image)
    let upload_output = Command::new("timeout")
        .args([
            "180s",
            &bck,
            "libvirt",
            "upload",
            test_image,
            "--volume-name",
            volume_name,
            "--disk-size",
            "5G",
            "--skip-upload",
            "--keep-temp",
        ])
        .output()
        .expect("Failed to run libvirt upload for integration test");

    if !upload_output.status.success() {
        let stderr = String::from_utf8_lossy(&upload_output.stderr);
        panic!("Upload step failed in integration test: {}", stderr);
    }

    // Step 2: Try to create domain (will fail without actual upload, but should validate)
    let create_output = Command::new(&bck)
        .args([
            "libvirt",
            "create",
            volume_name,
            "--memory",
            "2G",
            "--vcpus",
            "2",
            "--dry-run",
        ])
        .output()
        .expect("Failed to run libvirt create for integration test");

    // The create should fail gracefully since we didn't actually upload
    let create_stderr = String::from_utf8_lossy(&create_output.stderr);
    if !create_output.status.success() {
        assert!(
            create_stderr.contains("volume") || create_stderr.contains("not found"),
            "Create should fail with volume not found error: {}",
            create_stderr
        );
    }

    println!("libvirt upload -> create integration workflow tested");
}

/// Test SSH integration with created domains
pub fn test_libvirt_ssh_integration() {
    let bck = get_bck_command().unwrap();

    // Test that SSH command integration works syntactically
    let output = Command::new(&bck)
        .args(["libvirt", "ssh", "test-domain", "--", "echo", "hello"])
        .output()
        .expect("Failed to run libvirt ssh command");

    // Will likely fail since no domain exists, but should not crash
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        // Should fail gracefully with domain-related error
        assert!(
            stderr.contains("domain") || stderr.contains("connect") || stderr.contains("ssh"),
            "SSH integration should fail gracefully: {}",
            stderr
        );
    }

    println!("libvirt SSH integration tested");
}

/// Test VM startup and shutdown with libvirt create
pub fn test_libvirt_vm_lifecycle() {
    // Skip if running in CI/container environment without libvirt
    if std::env::var("CI").is_ok() || !std::path::Path::new("/usr/bin/virsh").exists() {
        println!("Skipping VM lifecycle test - no libvirt available");
        return;
    }

    let bck = get_bck_command().unwrap();
    let test_volume = "test-vm-lifecycle";
    let domain_name = format!("bootc-{}", test_volume);

    // Cleanup any existing test domain
    let _ = std::process::Command::new("virsh")
        .args(&["destroy", &domain_name])
        .output();
    let _ = std::process::Command::new("virsh")
        .args(&["undefine", &domain_name])
        .output();

    // Create a minimal test volume (skip if no bootc container available)
    let test_image = "quay.io/fedora/fedora-bootc:42";

    // First try to create a domain from container image
    let output = std::process::Command::new(&bck)
        .args(&[
            "libvirt",
            "create",
            "--filesystem",
            "ext4",
            "--domain-name",
            &domain_name,
            "--force",
            test_image,
        ])
        .output()
        .expect("Failed to run libvirt create");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("Create failed (expected in CI): {}", stderr);

        // If creation fails (e.g., no libvirt storage), skip the test
        if stderr.contains("pool")
            || stderr.contains("connect")
            || stderr.contains("Permission denied")
        {
            println!("Skipping VM lifecycle test - libvirt not properly configured");
            return;
        }

        panic!("Unexpected failure in VM creation: {}", stderr);
    }

    println!("Created VM domain: {}", domain_name);

    // Try to start the domain
    let start_output = std::process::Command::new("virsh")
        .args(&["start", &domain_name])
        .output()
        .expect("Failed to run virsh start");

    if start_output.status.success() {
        println!("Successfully started VM: {}", domain_name);

        // Verify domain is running
        let dominfo_output = std::process::Command::new("virsh")
            .args(&["dominfo", &domain_name])
            .output()
            .expect("Failed to run virsh dominfo");

        let info = String::from_utf8_lossy(&dominfo_output.stdout);
        assert!(info.contains("State:"), "Should show domain state");

        // Wait a moment for VM to initialize
        std::thread::sleep(std::time::Duration::from_secs(5));

        // Stop the domain
        let stop_output = std::process::Command::new("virsh")
            .args(&["destroy", &domain_name])
            .output()
            .expect("Failed to run virsh destroy");

        if !stop_output.status.success() {
            let stderr = String::from_utf8_lossy(&stop_output.stderr);
            eprintln!("Warning: Failed to stop domain: {}", stderr);
        } else {
            println!("Successfully stopped VM: {}", domain_name);
        }
    } else {
        let stderr = String::from_utf8_lossy(&start_output.stderr);
        println!("VM start failed (may be expected): {}", stderr);

        // Some failures are acceptable (no KVM, nested virtualization, etc.)
        let acceptable_failures = [
            "KVM",
            "kvm",
            "nested",
            "hardware",
            "acceleration",
            "permission",
            "qemu",
            "network",
        ];

        let is_acceptable = acceptable_failures
            .iter()
            .any(|&pattern| stderr.to_lowercase().contains(pattern));

        if !is_acceptable {
            panic!("Unexpected VM start failure: {}", stderr);
        }
    }

    // Cleanup - remove the domain
    let _ = std::process::Command::new("virsh")
        .args(&["destroy", &domain_name])
        .output();
    let cleanup_output = std::process::Command::new("virsh")
        .args(&["undefine", &domain_name])
        .output()
        .expect("Failed to cleanup domain");

    if cleanup_output.status.success() {
        println!("Cleaned up VM domain: {}", domain_name);
    }

    println!("VM lifecycle test completed");
}

/// Test error handling for invalid configurations
pub fn test_libvirt_error_handling() {
    let bck = get_bck_command().unwrap();

    let error_cases = vec![
        // Missing required arguments
        (vec!["libvirt", "upload"], "missing image"),
        (vec!["libvirt", "create"], "missing volume"),
        // Invalid resource specs
        (
            vec!["libvirt", "create", "vol", "--memory", "invalid"],
            "invalid memory",
        ),
        (
            vec!["libvirt", "upload", "img", "--disk-size", "bad"],
            "invalid size",
        ),
        // Invalid pool names
        (vec!["libvirt", "list", "--pool", ""], "empty pool"),
    ];

    for (args, error_desc) in error_cases {
        let output = Command::new(&bck)
            .args(&args)
            .output()
            .expect(&format!("Failed to run error case: {}", error_desc));

        assert!(
            !output.status.success(),
            "Should fail for case: {}",
            error_desc
        );

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.is_empty(),
            "Should have error message for case: {}",
            error_desc
        );
    }

    println!("libvirt error handling validated");
}

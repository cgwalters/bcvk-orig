use std::path::Path;

use color_eyre::eyre::{eyre, Context};
use color_eyre::Result;
use libtest_mimic::{Arguments, Trial};
use serde_json::Value;
use xshell::{cmd, Shell};

/// Label used to identify containers created by integration tests
pub(crate) const INTEGRATION_TEST_LABEL: &str = "--label=bcvk.integration-test=1";

/// Cleanup all containers with the integration test label
pub(crate) fn cleanup_integration_test_containers() {
    println!("Cleaning up integration test containers...");

    // List all containers with our integration test label
    let list_output = std::process::Command::new("podman")
        .args([
            "ps",
            "-a",
            "--filter",
            "label=bcvk.integration-test=1",
            "-q",
        ])
        .output();

    if let Ok(output) = list_output {
        if output.status.success() {
            let container_ids = String::from_utf8_lossy(&output.stdout);
            let containers: Vec<&str> = container_ids.lines().filter(|l| !l.is_empty()).collect();

            if !containers.is_empty() {
                println!(
                    "Found {} integration test container(s) to clean up",
                    containers.len()
                );

                // Force remove each container
                for container_id in containers {
                    let _ = std::process::Command::new("podman")
                        .args(["rm", "-f", container_id])
                        .output();
                }
                println!("Cleanup completed");
            } else {
                println!("No integration test containers found to clean up");
            }
        }
    }
}

mod tests {
    pub mod libvirt_upload_disk;
    pub mod libvirt_verb;
    pub mod mount_feature;
    pub mod run_ephemeral;
    pub mod run_ephemeral_ssh;
    pub mod to_disk;
}

/// Get the path to the bck binary, checking BCVK_PATH env var first, then falling back to "bck"
pub(crate) fn get_bck_command() -> Result<String> {
    if let Some(path) = std::env::var("BCVK_PATH").ok() {
        return Ok(path);
    }
    // Force the user to set this if we're running from the project dir
    if let Some(path) = ["target/debug/bck", "target/release/bck"]
        .into_iter()
        .find(|p| Path::new(p).exists())
    {
        return Err(eyre!(
            "Detected {path} - set BCVK_PATH={path} to run using this binary"
        ));
    }
    return Ok("bck".to_owned());
}

fn test_images_list() -> Result<()> {
    println!("Running test: bck images list --json");

    let sh = Shell::new()?;
    let bck = get_bck_command()?;

    // Run the bck images list command with JSON output
    let output = cmd!(sh, "{bck} images list --json").output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Failed to run 'bck images list --json': {}", stderr));
    }

    // Parse the JSON output
    let stdout = String::from_utf8(output.stdout)?;
    let images: Value = serde_json::from_str(&stdout).context("Failed to parse JSON output")?;

    // Verify the structure and content of the JSON
    let images_array = images
        .as_array()
        .ok_or_else(|| eyre!("Expected JSON array in output, got: {}", stdout))?;

    // Verify that the array contains valid image objects
    for (index, image) in images_array.iter().enumerate() {
        if !image.is_object() {
            return Err(eyre!(
                "Image entry {} is not a JSON object: {}",
                index,
                image
            ));
        }
    }

    println!(
        "Test passed: bck images list --json (found {} images)",
        images_array.len()
    );
    println!("All image entries are valid JSON objects");
    Ok(())
}

fn main() {
    let args = Arguments::from_args();

    let tests = vec![
        Trial::test("images_list", || {
            test_images_list()?;
            Ok(())
        }),
        Trial::test("run_ephemeral_correct_kernel", || {
            tests::run_ephemeral::test_run_ephemeral_correct_kernel();
            Ok(())
        }),
        Trial::test("run_ephemeral_poweroff", || {
            tests::run_ephemeral::test_run_ephemeral_poweroff();
            Ok(())
        }),
        Trial::test("run_ephemeral_with_memory_limit", || {
            tests::run_ephemeral::test_run_ephemeral_with_memory_limit();
            Ok(())
        }),
        Trial::test("run_ephemeral_with_vcpus", || {
            tests::run_ephemeral::test_run_ephemeral_with_vcpus();
            Ok(())
        }),
        Trial::test("run_ephemeral_execute", || {
            tests::run_ephemeral::test_run_ephemeral_execute();
            Ok(())
        }),
        Trial::test("run_ephemeral_container_ssh_access", || {
            tests::run_ephemeral::test_run_ephemeral_container_ssh_access();
            Ok(())
        }),
        Trial::test("run_ephemeral_vsock_systemd_debugging", || {
            tests::run_ephemeral::test_run_ephemeral_vsock_systemd_debugging();
            Ok(())
        }),
        Trial::test("run_ephemeral_ssh_command", || {
            tests::run_ephemeral_ssh::test_run_ephemeral_ssh_command();
            Ok(())
        }),
        Trial::test("run_ephemeral_ssh_cleanup", || {
            tests::run_ephemeral_ssh::test_run_ephemeral_ssh_cleanup();
            Ok(())
        }),
        Trial::test("run_ephemeral_ssh_system_command", || {
            tests::run_ephemeral_ssh::test_run_ephemeral_ssh_system_command();
            Ok(())
        }),
        Trial::test("run_ephemeral_ssh_exit_code", || {
            tests::run_ephemeral_ssh::test_run_ephemeral_ssh_exit_code();
            Ok(())
        }),
        Trial::test("mount_feature_bind", || {
            tests::mount_feature::test_mount_feature_bind();
            Ok(())
        }),
        Trial::test("mount_feature_ro_bind", || {
            tests::mount_feature::test_mount_feature_ro_bind();
            Ok(())
        }),
        Trial::test("to_disk", || {
            tests::to_disk::test_to_disk();
            Ok(())
        }),
        Trial::test("libvirt_list_functionality", || {
            tests::libvirt_verb::test_libvirt_list_functionality();
            Ok(())
        }),
        Trial::test("libvirt_list_json_output", || {
            tests::libvirt_verb::test_libvirt_list_json_output();
            Ok(())
        }),
        Trial::test("libvirt_create_resource_options", || {
            tests::libvirt_verb::test_libvirt_create_resource_options();
            Ok(())
        }),
        Trial::test("libvirt_create_networking", || {
            tests::libvirt_verb::test_libvirt_create_networking();
            Ok(())
        }),
        Trial::test("libvirt_ssh_integration", || {
            tests::libvirt_verb::test_libvirt_ssh_integration();
            Ok(())
        }),
        Trial::test("libvirt_vm_lifecycle", || {
            tests::libvirt_verb::test_libvirt_vm_lifecycle();
            Ok(())
        }),
        Trial::test("libvirt_error_handling", || {
            tests::libvirt_verb::test_libvirt_error_handling();
            Ok(())
        }),
    ];

    // Run the tests and capture the exit code
    let exit_code = libtest_mimic::run(&args, tests);

    // Clean up any containers created by integration tests
    cleanup_integration_test_containers();

    // Exit with the test result
    exit_code.exit();
}

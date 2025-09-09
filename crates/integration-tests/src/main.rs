use std::path::Path;

use color_eyre::eyre::{eyre, Context};
use color_eyre::Result;
use libtest_mimic::{Arguments, Trial};
use serde_json::Value;
use xshell::{cmd, Shell};

mod tests {
    pub mod mount_feature;
    pub mod run_ephemeral;
    pub mod run_install;
}

/// Get the path to the bck binary, checking BCK_PATH env var first, then falling back to "bck"
pub(crate) fn get_bck_command() -> Result<String> {
    if let Some(path) = std::env::var("BCK_PATH").ok() {
        return Ok(path);
    }
    // Force the user to set this if we're running from the project dir
    if let Some(path) = ["target/debug/bck", "target/release/bck"]
        .into_iter()
        .find(|p| Path::new(p).exists())
    {
        return Err(eyre!(
            "Detected {path} - set BCK_PATH={path} to run using this binary"
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
        Trial::test("run_ephemeral_execute_stress", || {
            tests::run_ephemeral::test_run_ephemeral_execute_stress();
            Ok(())
        }),
        Trial::test("run_ephemeral_ssh_key_generation", || {
            tests::run_ephemeral::test_run_ephemeral_ssh_key_generation();
            Ok(())
        }),
        Trial::test("run_ephemeral_ssh_credential_injection", || {
            tests::run_ephemeral::test_run_ephemeral_ssh_credential_injection();
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
        Trial::test("mount_feature_bind", || {
            tests::mount_feature::test_mount_feature_bind();
            Ok(())
        }),
        Trial::test("mount_feature_ro_bind", || {
            tests::mount_feature::test_mount_feature_ro_bind();
            Ok(())
        }),
        Trial::test("mount_feature_multiple", || {
            tests::mount_feature::test_mount_feature_multiple();
            Ok(())
        }),
        Trial::test("storage_detection", || {
            tests::run_install::test_storage_detection();
            Ok(())
        }),
        Trial::test("run_ephemeral_with_storage", || {
            tests::run_install::test_run_ephemeral_with_storage();
            Ok(())
        }),
        Trial::test("run_install_help", || {
            tests::run_install::test_run_install_help();
            Ok(())
        }),
        Trial::test("run_install_debug_mode", || {
            tests::run_install::test_run_install_debug_mode();
            Ok(())
        }),
        Trial::test("run_install_validation", || {
            tests::run_install::test_run_install_validation();
            Ok(())
        }),
        Trial::test("run_install_custom_storage_path", || {
            tests::run_install::test_run_install_custom_storage_path();
            Ok(())
        }),
        Trial::test("run_install_invalid_storage", || {
            tests::run_install::test_run_install_invalid_storage();
            Ok(())
        }),
        Trial::test("run_install_to_disk", || {
            tests::run_install::test_run_install_to_disk();
            Ok(())
        }),
        Trial::test("run_install_test_mode", || {
            tests::run_install::test_run_install_test_mode();
            Ok(())
        }),
        Trial::test("virtiofsd_startup_validation", || {
            tests::run_install::test_virtiofsd_startup_validation();
            Ok(())
        }),
    ];

    libtest_mimic::run(&args, tests).exit();
}

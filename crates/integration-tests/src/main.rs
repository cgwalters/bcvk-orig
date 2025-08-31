use libtest_mimic::{Arguments, Failed, Trial};
use serde_json::Value;
use xshell::{cmd, Shell};

mod tests {
    pub mod mount_feature;
    pub mod run_ephemeral;
}

/// Get the path to the bck binary, checking BCK_PATH env var first, then falling back to "bck"
fn get_bck_command() -> String {
    std::env::var("BCK_PATH").unwrap_or_else(|_| "bck".to_string())
}

fn test_images_list() -> Result<(), Failed> {
    // Skip this test if we're not running in a container environment
    // The images list command requires special container privileges
    if std::env::var("CONTAINER").is_err() {
        println!("Skipping test: bck images list (requires container environment)");
        return Ok(());
    }

    println!("Running test: bck images list --json");

    let sh = Shell::new().map_err(|e| format!("Failed to create shell: {}", e))?;
    let bck = get_bck_command();

    // Run the bck images list command with JSON output
    let output = cmd!(sh, "{bck} images list --json")
        .output()
        .map_err(|e| format!("Failed to run command: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to run 'bck images list --json': {}", stderr).into());
    }

    // Parse the JSON output
    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| format!("Failed to parse stdout as UTF-8: {}", e))?;
    let images: Value =
        serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse JSON output: {}", e))?;

    // Verify the structure and content of the JSON
    if !images.is_array() {
        return Err(format!("Expected JSON array in output, got: {}", stdout).into());
    }

    let images_array = images.as_array().unwrap();
    if images_array.is_empty() {
        return Err("No images found in the JSON output".into());
    }

    println!(
        "Test passed: bck images list --json (found {} images)",
        images_array.len()
    );
    Ok(())
}

fn main() {
    let args = Arguments::from_args();

    let tests = vec![
        Trial::test("images_list", || test_images_list()),
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
    ];

    libtest_mimic::run(&args, tests).exit();
}

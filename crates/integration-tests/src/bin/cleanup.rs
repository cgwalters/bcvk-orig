use std::process::Command;

/// Label used to identify containers created by integration tests
const INTEGRATION_TEST_LABEL: &str = "bcvk.integration-test=1";

fn cleanup_integration_test_containers() -> Result<(), Box<dyn std::error::Error>> {
    println!("Cleaning up integration test containers...");

    // List all containers with our integration test label
    let list_output = Command::new("podman")
        .args([
            "ps",
            "-a",
            "--filter",
            &format!("label={}", INTEGRATION_TEST_LABEL),
            "-q",
        ])
        .output()?;

    if !list_output.status.success() {
        eprintln!("Warning: Failed to list containers");
        return Ok(());
    }

    let container_ids = String::from_utf8_lossy(&list_output.stdout);
    let containers: Vec<&str> = container_ids.lines().filter(|l| !l.is_empty()).collect();

    if containers.is_empty() {
        println!("No integration test containers found to clean up");
        return Ok(());
    }

    println!(
        "Found {} integration test container(s) to clean up",
        containers.len()
    );

    // Force remove each container
    let mut cleaned = 0;
    for container_id in containers {
        print!(
            "  Removing container {}... ",
            &container_id[..12.min(container_id.len())]
        );
        let rm_output = Command::new("podman")
            .args(["rm", "-f", container_id])
            .output()?;

        if rm_output.status.success() {
            println!("✓");
            cleaned += 1;
        } else {
            println!("✗ (failed)");
            eprintln!("    Error: {}", String::from_utf8_lossy(&rm_output.stderr));
        }
    }

    println!("Cleanup completed: {} container(s) removed", cleaned);
    Ok(())
}

fn main() {
    if let Err(e) = cleanup_integration_test_containers() {
        eprintln!("Error during cleanup: {}", e);
        std::process::exit(1);
    }
}

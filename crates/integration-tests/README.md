# Integration Tests for bootc-kit

This crate contains comprehensive integration tests for bootc-kit functionality.

## Test Structure

### Core Tests
- `run_ephemeral.rs` - Tests ephemeral VM functionality with various configurations
- `run_install.rs` - Tests installation workflows and disk operations
- `mount_feature.rs` - Tests virtiofs mount features (bind, ro-bind, multiple mounts)
- `bootc_install_workflow.rs` - Tests bootc installation workflow components

### Running Tests

```bash
# Run all integration tests
cargo run --bin integration-tests

# Run specific test by name
cargo run --bin integration-tests <test_name>

# List available tests
cargo run --bin integration-tests --list

# Or use the project's just command
just test-integration
```

### Environment Setup

Tests can use either the installed `bck` binary or the development binary:

```bash
# Use specific binary path
export BCK_PATH="/path/to/bootc-kit"

# Or let tests auto-detect development binary
# (tests will use target/debug/bootc-kit if available, falling back to 'bck')
```

## Bootc Install Workflow Tests

The `bootc_install_workflow.rs` tests validate all components needed for bootc installation in ephemeral VMs.

### Current Capabilities
These tests validate prerequisite components:

1. **Container storage access** via `--bind-storage-ro`
2. **bootc command availability** in target container images
3. **systemd units injection** for custom installation workflows
4. **Disk image creation** and management utilities
5. **Error handling** for various failure scenarios

### Test Coverage
- `test_bootc_install_workflow()` - Comprehensive workflow validation with multiple VM tests
- `test_bootc_install_workflow_quick()` - Fast prerequisites check for CI environments
- `test_bootc_install_workflow_error_handling()` - Error scenarios and graceful failure handling

### Architecture
The tests demonstrate the intended bootc installation workflow:

```bash
bootc-kit run-ephemeral \
  --mount-disk-file /path/to/disk.img:output \
  --bind-storage-ro \
  --execute "bootc install to-disk --source-imgref <image> /dev/disk/by-id/virtio-output" \
  <image>
```

### Future Enhancements
When `--mount-disk-file` is fully stabilized, tests can be enhanced to:
- Mount disk files as virtio-blk devices in VMs
- Perform actual bootc installations to disk images
- Validate installed disk contents and partition structures
- Test complete end-to-end installation workflows

## Adding New Tests

To add integration tests, follow the libtest-mimic pattern used in `main.rs`:

1. Create test functions in appropriate module files under `src/tests/`
2. Add test trials to the main function in `main.rs`
3. Use the existing helper functions and patterns for consistency

Example test structure:
```rust
pub fn test_new_feature() {
    let bck = get_bck_command()?;
    
    let output = Command::new("timeout")
        .args(["60s", &bck, "new-feature", "--test"])
        .output()
        .expect("Failed to run test command");
    
    assert!(output.status.success(), "Command failed: {}", 
            String::from_utf8_lossy(&output.stderr));
    
    println!("New feature test passed");
}
```

## Test Requirements

### System Requirements
- QEMU/KVM virtualization support
- Container runtime (podman) installed and configured
- 2GB+ available disk space for test artifacts
- Internet access for pulling test container images

### Container Images
Tests use these bootc-enabled container images:
- `quay.io/fedora/fedora-bootc:42` - Primary Fedora-based test image
- `quay.io/centos-bootc/centos-bootc:stream10` - CentOS Stream test image

### Performance Considerations
- Individual tests have timeouts (typically 60-300s)
- VM-based tests require more resources and time
- Use `*_quick` variants for faster CI testing
- Tests clean up temporary files automatically

## Troubleshooting

### Common Issues

1. **QEMU fails to start**: Verify virtualization support with `kvm-ok` or similar
2. **Tests timeout**: Increase timeout values for slower systems  
3. **Image pull failures**: Check network connectivity and container registry access
4. **Permission errors**: Ensure proper SELinux/AppArmor configuration for containers

### Debug Output
Enable verbose logging for troubleshooting:

```bash
# Debug bootc-kit operations
RUST_LOG=debug cargo run --bin integration-tests <test_name>

# Debug with backtraces
RUST_BACKTRACE=1 cargo run --bin integration-tests <test_name>
```

### Test Isolation
Each test uses temporary directories and should be isolated, but some VM-based tests may:
- Use system virtualization resources
- Require elevated permissions for some operations
- Take significant time to complete

For reliable CI testing, consider running VM-intensive tests separately or with increased timeouts.
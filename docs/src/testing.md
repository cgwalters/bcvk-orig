# Testing

This guide covers how to test bcvk, including unit tests, integration tests, and manual testing procedures.

## Test Overview

bcvk includes comprehensive testing to ensure reliability and functionality:

- **Unit tests**: Test individual components and functions
- **Integration tests**: Test complete workflows with real VMs
- **Manual tests**: Human verification of functionality
- **CI/CD tests**: Automated testing in continuous integration

## Prerequisites for Testing

### Basic Requirements

```bash
# Development tools (see building.md for details)
cargo --version
rustc --version

# Runtime dependencies
qemu-system-x86_64 --version
podman --version
```

### Integration Test Requirements

Integration tests require a full virtualization environment:

```bash
# Verify KVM support
ls -la /dev/kvm
lsmod | grep kvm

# Check virtualization
virt-host-validate qemu

# Test basic QEMU functionality
qemu-system-x86_64 -version
```

### libvirt Integration Tests

For libvirt-related tests:

```bash
# Install libvirt
sudo systemctl status libvirtd

# Verify user permissions
groups | grep libvirt

# Test libvirt connection
virsh list --all
```

## Running Tests

### Quick Test Commands

Using the project's `Justfile`:

```bash
# Install just if not available
cargo install just

# Run all unit tests
just test

# Run integration tests
just test-integration

# Run specific integration test
just test-integration-single test_name

# Run all tests with cleanup
just test && just test-integration
```

### Manual Test Commands

Using cargo directly:

```bash
# Unit tests only
cargo test

# All tests including integration
cargo test --all

# Specific test file
cargo test --test integration

# Specific test function
cargo test test_ephemeral_vm_creation

# Run tests with output
cargo test -- --nocapture

# Run tests single-threaded (for debugging)
cargo test -- --test-threads=1
```

## Unit Tests

### Running Unit Tests

```bash
# All unit tests
cargo test --lib

# Specific module
cargo test config::tests

# Test with specific features
cargo test --features libvirt

# Show test output
cargo test -- --nocapture
```

### Writing Unit Tests

Unit tests should be placed in the same file as the code they test:

```rust
// src/vm/config.rs
pub struct VmConfig {
    pub memory: u64,
    pub cpus: u32,
}

impl VmConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.memory == 0 {
            return Err("Memory must be greater than 0".to_string());
        }
        if self.cpus == 0 {
            return Err("CPUs must be greater than 0".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_config() {
        let config = VmConfig {
            memory: 2048,
            cpus: 2,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_zero_memory() {
        let config = VmConfig {
            memory: 0,
            cpus: 2,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_zero_cpus() {
        let config = VmConfig {
            memory: 2048,
            cpus: 0,
        };
        assert!(config.validate().is_err());
    }
}
```

### Test Organization

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    // Helper functions for tests
    fn create_test_config() -> VmConfig {
        VmConfig {
            memory: 2048,
            cpus: 2,
        }
    }
    
    // Group related tests
    mod config_validation {
        use super::*;
        
        #[test]
        fn valid_configuration() {
            // Test implementation
        }
        
        #[test]
        fn invalid_memory_size() {
            // Test implementation
        }
    }
    
    mod vm_creation {
        use super::*;
        
        #[test]
        fn successful_creation() {
            // Test implementation
        }
    }
}
```

## Integration Tests

### Available Integration Tests

View available integration tests:

```bash
# List integration test files
ls tests/

# Common integration tests:
# - ephemeral_vm.rs: Test ephemeral VM functionality
# - to_disk.rs: Test disk image creation
# - libvirt.rs: Test libvirt integration
# - ssh_access.rs: Test SSH connectivity
```

### Running Integration Tests

```bash
# All integration tests
cargo test --test integration

# Specific integration test file
cargo test --test ephemeral_vm

# Specific test function
cargo test --test integration test_ephemeral_run_basic

# With verbose output
cargo test --test integration -- --nocapture

# Single test with full output
cargo test --test integration test_ephemeral_ssh -- --nocapture --test-threads=1
```

### Integration Test Examples

Example integration test structure:

```rust
// tests/ephemeral_vm.rs
use std::process::Command;
use std::time::Duration;

#[test]
fn test_ephemeral_run_basic() {
    let output = Command::new("./target/debug/bcvk")
        .args(&["ephemeral", "run", "-d", "--rm", "--name", "test-vm", "quay.io/fedora/fedora-bootc:42"])
        .output()
        .expect("Failed to execute command");
    
    assert!(output.status.success(), "VM creation failed: {}", String::from_utf8_lossy(&output.stderr));
    
    // Clean up
    let _ = Command::new("podman")
        .args(&["stop", "test-vm"])
        .output();
}

#[test]
fn test_ephemeral_ssh() {
    // Create VM with SSH keys
    let create_output = Command::new("./target/debug/bcvk")
        .args(&["ephemeral", "run", "-d", "-K", "--name", "ssh-test", "quay.io/fedora/fedora-bootc:42"])
        .output()
        .expect("Failed to create VM");
    
    assert!(create_output.status.success());
    
    // Wait for VM to boot
    std::thread::sleep(Duration::from_secs(30));
    
    // Test SSH connection
    let ssh_output = Command::new("./target/debug/bcvk")
        .args(&["ephemeral", "ssh", "ssh-test", "echo", "test"])
        .output()
        .expect("Failed to SSH");
    
    assert!(ssh_output.status.success());
    assert_eq!(String::from_utf8_lossy(&ssh_output.stdout).trim(), "test");
    
    // Clean up
    let _ = Command::new("podman")
        .args(&["stop", "ssh-test"])
        .output();
}
```

### Integration Test Requirements

Integration tests need:

1. **Container images**: Access to bootc container images
2. **Network access**: To pull container images
3. **Virtualization**: Working KVM/QEMU setup
4. **Disk space**: For VM images and containers
5. **Time**: VMs take time to boot

### Test Image Requirements

Integration tests use specific test images:

```bash
# Pull required test images
podman pull quay.io/fedora/fedora-bootc:42
podman pull quay.io/centos-bootc/centos-bootc:stream10

# Verify images are bootc-compatible
bcvk images list
```

## Manual Testing

### Basic Functionality Testing

Test core features manually:

```bash
# 1. Version and help
./target/release/bcvk --version
./target/release/bcvk --help

# 2. Image management
./target/release/bcvk images list

# 3. Ephemeral VM (quick test)
./target/release/bcvk ephemeral run -d --rm --name manual-test quay.io/fedora/fedora-bootc:42
sleep 30
podman ps | grep manual-test
podman stop manual-test

# 4. SSH access
./target/release/bcvk ephemeral run -d -K --name ssh-manual quay.io/fedora/fedora-bootc:42
sleep 30
./target/release/bcvk ephemeral ssh ssh-manual "hostname"
podman stop ssh-manual
```

### Disk Image Testing

```bash
# Create disk image
./target/release/bcvk to-disk quay.io/fedora/fedora-bootc:42 /tmp/test-manual.img

# Verify image
file /tmp/test-manual.img
ls -lh /tmp/test-manual.img

# Test with QEMU (if possible)
qemu-system-x86_64 -hda /tmp/test-manual.img -m 2048 -enable-kvm -nographic

# Clean up
rm -f /tmp/test-manual.img
```

### libvirt Testing

```bash
# Test libvirt integration (if available)
./target/release/bcvk libvirt run --name libvirt-manual quay.io/fedora/fedora-bootc:42

# Check VM status
./target/release/bcvk libvirt list

# SSH test
./target/release/bcvk libvirt ssh libvirt-manual "uptime"

# Clean up
./target/release/bcvk libvirt stop libvirt-manual
./target/release/bcvk libvirt rm libvirt-manual
```

## Test Data Management

### Test Images

Manage test container images:

```bash
# Pull standard test images
podman pull quay.io/fedora/fedora-bootc:42
podman pull quay.io/centos-bootc/centos-bootc:stream10

# Create minimal test image (for faster testing)
cat > Containerfile.test <<EOF
FROM quay.io/fedora/fedora-bootc:42
LABEL containers.bootc=1
RUN echo "test image" > /root/test-marker
EOF

podman build -t localhost/bootc-test:latest -f Containerfile.test .
```

### Test Environment Cleanup

Clean up test artifacts:

```bash
# Stop and remove test VMs
podman ps -a | grep test | awk '{print $1}' | xargs -r podman rm -f

# Clean up test images
rm -f /tmp/test-*.img /tmp/test-*.qcow2

# Clean up libvirt test VMs
virsh list --all | grep test | awk '{print $2}' | xargs -r virsh destroy
virsh list --all | grep test | awk '{print $2}' | xargs -r virsh undefine

# Clean up containers
podman system prune -f
```

## Performance Testing

### Benchmark Tests

Create performance benchmarks:

```rust
// benches/vm_creation.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use bootc_kit::vm::VmConfig;

fn benchmark_vm_config_creation(c: &mut Criterion) {
    c.bench_function("vm_config_creation", |b| {
        b.iter(|| {
            let config = VmConfig::new()
                .memory(black_box(2048))
                .cpus(black_box(2));
            black_box(config)
        })
    });
}

criterion_group!(benches, benchmark_vm_config_creation);
criterion_main!(benches);
```

Run benchmarks:

```bash
# Install criterion
cargo install cargo-criterion

# Run benchmarks
cargo bench

# Compare benchmarks
cargo bench -- --save-baseline main
# Make changes, then:
cargo bench -- --baseline main
```

### Load Testing

Test with multiple concurrent operations:

```bash
#!/bin/bash
# load_test.sh

# Start multiple VMs concurrently
for i in {1..5}; do
    ./target/release/bcvk ephemeral run -d --rm --name "load-test-$i" quay.io/fedora/fedora-bootc:42 &
done

# Wait for all to start
wait

# Check all are running
podman ps | grep load-test

# Clean up
for i in {1..5}; do
    podman stop "load-test-$i" &
done
wait
```

## Continuous Integration Testing

### GitHub Actions

The project includes CI testing:

```yaml
# .github/workflows/test.yml
name: Tests

on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run unit tests
        run: cargo test --lib

  integration-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Install dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y qemu-kvm podman
      - name: Run integration tests
        run: cargo test --test integration
```

### Local CI Simulation

Simulate CI environment locally:

```bash
# Run in clean environment (using Docker)
docker run --rm -v $(pwd):/workspace -w /workspace rust:1.70 \
  bash -c "cargo test"

# Test with different Rust versions
docker run --rm -v $(pwd):/workspace -w /workspace rust:1.68 \
  bash -c "cargo check"
```

## Debugging Test Failures

### Common Test Issues

1. **VM won't start**:
   ```bash
   # Check KVM access
   ls -la /dev/kvm
   
   # Check QEMU installation
   qemu-system-x86_64 --version
   
   # Run with debug output
   RUST_LOG=debug cargo test test_name -- --nocapture
   ```

2. **SSH connection fails**:
   ```bash
   # Check if VM is actually running
   podman ps | grep test-vm
   
   # Check VM logs
   podman logs test-vm
   
   # Wait longer for boot
   sleep 60  # Instead of 30
   ```

3. **Disk space issues**:
   ```bash
   # Check available space
   df -h /tmp
   
   # Clean up old test artifacts
   rm -f /tmp/test-*.img
   podman system prune
   ```

### Debug Test Execution

```bash
# Run single test with full output
cargo test test_name -- --nocapture --test-threads=1

# Run with debug logging
RUST_LOG=debug cargo test test_name

# Run with environment variables
BCVK_TEST_TIMEOUT=120 cargo test test_name

# Run with GDB
rust-gdb --args target/debug/deps/test_binary test_name
```

### Test Isolation

Ensure tests don't interfere with each other:

```rust
// Use unique names for test resources
#[test]
fn test_vm_creation() {
    let test_id = format!("test-{}", std::process::id());
    let vm_name = format!("vm-{}", test_id);
    
    // Use unique name for VM
    create_vm(&vm_name, &config);
    
    // Clean up at end
    cleanup_vm(&vm_name);
}

// Use temporary directories
#[test]
fn test_disk_creation() {
    let temp_dir = tempfile::tempdir().unwrap();
    let disk_path = temp_dir.path().join("test.img");
    
    create_disk_image(&disk_path);
    
    // temp_dir is automatically cleaned up
}
```

## Test Coverage

### Measuring Coverage

```bash
# Install coverage tools
cargo install cargo-tarpaulin

# Run coverage analysis
cargo tarpaulin --out Html

# View coverage report
open tarpaulin-report.html
```

### Coverage Goals

Maintain good test coverage for:

- **Core functionality**: VM creation, disk images, SSH access
- **Error handling**: Invalid inputs, resource failures
- **Edge cases**: Boundary conditions, unusual configurations
- **Integration points**: Container runtime, QEMU, libvirt

## Test Documentation

### Writing Test Documentation

Document test purposes and requirements:

```rust
/// Tests that ephemeral VMs can be created successfully.
/// 
/// Requirements:
/// - QEMU/KVM available
/// - bootc container image accessible
/// - Sufficient disk space for VM
/// 
/// This test verifies:
/// - VM creation succeeds
/// - VM starts properly
/// - VM can be stopped and cleaned up
#[test]
fn test_ephemeral_vm_lifecycle() {
    // Test implementation
}
```

### Test Maintenance

Keep tests updated:

1. **Update test images** when base images change
2. **Adjust timeouts** based on performance changes
3. **Add tests** for new features
4. **Remove obsolete tests** for deprecated functionality
5. **Update documentation** when test requirements change

## Best Practices

### Test Design

1. **Make tests deterministic**: Avoid flaky tests
2. **Use meaningful names**: Clear test purpose
3. **Test one thing**: Focus each test
4. **Clean up resources**: Prevent test pollution
5. **Handle timeouts**: VMs need time to boot

### Resource Management

1. **Limit concurrent tests**: Avoid resource contention
2. **Use minimal resources**: Faster test execution
3. **Clean up thoroughly**: Prevent disk space issues
4. **Monitor system resources**: Ensure adequate capacity

### Troubleshooting

1. **Run tests individually**: Isolate failures
2. **Check system resources**: Ensure adequate CPU/memory/disk
3. **Verify dependencies**: Ensure all tools are installed
4. **Check logs**: Review detailed output
5. **Update test environment**: Keep tools current

## Next Steps

After setting up testing:

1. **Run the full test suite** to ensure everything works
2. **Contribute tests** for any missing functionality
3. **Set up CI/CD** if working on a fork
4. **Read the [contributing guide](./contributing.md)** for development workflow
5. **Explore the [architecture](./architecture.md)** to understand implementation details
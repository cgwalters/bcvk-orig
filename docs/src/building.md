# Building from Source

This guide covers how to build bcvk from source code, including development setup, compilation, and testing.

## Prerequisites

Before building bcvk, ensure you have the required tools and dependencies installed.

### Required Tools

#### Rust Toolchain
bcvk is written in Rust and requires a recent Rust toolchain:

```bash
# Install Rust via rustup (recommended)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Reload your shell environment
source ~/.cargo/env

# Verify installation
rustc --version
cargo --version

# Update to latest stable (if already installed)
rustup update stable
```

#### Git
Required for cloning the repository:

```bash
# On Fedora/RHEL/CentOS
sudo dnf install git

# On Ubuntu/Debian
sudo apt install git

# On macOS
brew install git
```

#### Build Tools
Additional tools needed for compilation:

```bash
# On Fedora/RHEL/CentOS
sudo dnf install gcc pkg-config openssl-devel

# On Ubuntu/Debian
sudo apt install build-essential pkg-config libssl-dev

# On macOS
xcode-select --install
```

### Runtime Dependencies

These are needed for running bcvk after building:

```bash
# On Fedora/RHEL/CentOS
sudo dnf install qemu-kvm qemu-system-x86 qemu-img podman virtiofsd

# On Ubuntu/Debian
sudo apt install qemu-kvm qemu-system-x86 qemu-utils podman virtiofsd

# On macOS
brew install qemu podman
```

### Optional Dependencies

For full functionality, install these optional components:

```bash
# libvirt (for libvirt integration features)
# On Fedora/RHEL/CentOS
sudo dnf install libvirt libvirt-daemon-kvm virt-install
sudo systemctl enable --now libvirtd

# On Ubuntu/Debian
sudo apt install libvirt-daemon-system libvirt-clients virtinst
sudo systemctl enable --now libvirtd

# Add user to libvirt group
sudo usermod -a -G libvirt $USER
```

## Getting the Source Code

### Clone the Repository

```bash
# Clone the main repository
git clone https://github.com/cgwalters/bcvk.git
cd bcvk

# Check available branches and tags
git branch -a
git tag -l
```

### Development vs. Release Builds

```bash
# For development (latest changes)
git checkout main

# For specific release
git checkout v0.1.0  # Replace with actual version

# For contributing (create feature branch)
git checkout -b feature/my-new-feature
```

## Build Process

### Quick Build

For a basic development build:

```bash
# Debug build (faster compilation, includes debug symbols)
cargo build

# The binary will be at: target/debug/bcvk
./target/debug/bcvk --version
```

### Release Build

For optimized production builds:

```bash
# Release build (optimized, smaller binary)
cargo build --release

# The binary will be at: target/release/bcvk
./target/release/bcvk --version
```

### Build with Specific Features

bcvk may have optional features that can be enabled or disabled:

```bash
# Build with all features
cargo build --release --all-features

# Build with specific features
cargo build --release --features "libvirt,cloud-support"

# Build without default features
cargo build --release --no-default-features

# List available features
cargo metadata --format-version 1 | jq '.packages[0].features'
```

## Build Configuration

### Cargo Configuration

You can customize the build process by creating a `.cargo/config.toml` file:

```toml
[build]
# Use all available CPU cores for compilation
jobs = 0

# Target specific architecture
target = "x86_64-unknown-linux-gnu"

[target.x86_64-unknown-linux-gnu]
# Use specific linker
linker = "gcc"

# Optimize for size
[profile.release]
opt-level = "s"
lto = true
codegen-units = 1
panic = "abort"
```

### Environment Variables

Control the build with environment variables:

```bash
# Use specific target
export CARGO_BUILD_TARGET=x86_64-unknown-linux-gnu

# Enable verbose output
export CARGO_BUILD_VERBOSE=true

# Use specific number of parallel jobs
export CARGO_BUILD_JOBS=4

# Custom build directory
export CARGO_TARGET_DIR=/fast-storage/target

# Run build
cargo build --release
```

## Cross-Compilation

### Building for Different Architectures

```bash
# Install cross-compilation targets
rustup target add x86_64-unknown-linux-gnu
rustup target add aarch64-unknown-linux-gnu
rustup target add x86_64-pc-windows-gnu

# Build for specific target
cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
```

### Cross-Compilation with Docker

For complex cross-compilation scenarios:

```bash
# Create Dockerfile for cross-compilation
cat > Dockerfile.cross <<EOF
FROM rust:1.70-bullseye

# Install cross-compilation tools
RUN apt-get update && apt-get install -y \\
    gcc-aarch64-linux-gnu \\
    pkg-config \\
    libssl-dev

# Set up cross-compilation environment
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc

WORKDIR /src
COPY . .

RUN rustup target add aarch64-unknown-linux-gnu
RUN cargo build --release --target aarch64-unknown-linux-gnu
EOF

# Build with Docker
docker build -f Dockerfile.cross -t bcvk-cross .
docker run --rm -v $(pwd)/target:/src/target bcvk-cross
```

## Testing the Build

### Unit Tests

```bash
# Run all unit tests
cargo test

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture

# Run tests in single thread (for debugging)
cargo test -- --test-threads=1
```

### Integration Tests

```bash
# Run integration tests (requires QEMU/KVM)
cargo test --test integration

# Run specific integration test
cargo test --test integration test_ephemeral_vm

# Run with verbose output
cargo test --test integration -- --nocapture
```

### Manual Testing

```bash
# Test basic functionality
./target/release/bcvk --version
./target/release/bcvk --help

# Test image listing (should work without VMs)
./target/release/bcvk images list

# Test ephemeral VM (requires bootc image)
./target/release/bcvk ephemeral run --help
```

## Development Tools

### Using Just (Recommended)

The project includes a `Justfile` for common development tasks:

```bash
# Install just
cargo install just

# View available commands
just --list

# Common development tasks
just check      # Check code without building
just build      # Build debug version
just test       # Run unit tests
just test-integration  # Run integration tests
just fmt        # Format code
just clippy     # Run clippy lints
just clean      # Clean build artifacts
```

### Manual Development Commands

If you prefer manual commands:

```bash
# Check code for errors without building
cargo check

# Format code
cargo fmt

# Run clippy for linting
cargo clippy

# Check for security advisories
cargo audit

# Update dependencies
cargo update

# Clean build artifacts
cargo clean
```

## Development Workflow

### Code Formatting

Always format code before committing:

```bash
# Format all code
cargo fmt

# Check formatting without modifying files
cargo fmt -- --check

# Format specific file
rustfmt src/main.rs
```

### Linting and Code Quality

```bash
# Run clippy with all targets
cargo clippy --all-targets --all-features

# Run clippy with strict settings
cargo clippy -- -D warnings

# Check for common mistakes
cargo clippy -- -W clippy::all -W clippy::pedantic
```

### Documentation

```bash
# Build documentation
cargo doc

# Build and open documentation
cargo doc --open

# Build documentation with private items
cargo doc --document-private-items

# Check documentation examples
cargo test --doc
```

## Troubleshooting Build Issues

### Common Build Problems

1. **Rust version too old**:
   ```bash
   # Update Rust
   rustup update stable
   rustup default stable
   
   # Check minimum supported version in Cargo.toml
   grep rust-version Cargo.toml
   ```

2. **Missing system dependencies**:
   ```bash
   # On Fedora/RHEL/CentOS
   sudo dnf install gcc pkg-config openssl-devel
   
   # On Ubuntu/Debian
   sudo apt install build-essential pkg-config libssl-dev
   ```

3. **OpenSSL linking issues**:
   ```bash
   # Set OpenSSL environment variables
   export OPENSSL_DIR=/usr/local/ssl
   export OPENSSL_LIB_DIR=/usr/local/ssl/lib
   export OPENSSL_INCLUDE_DIR=/usr/local/ssl/include
   
   # Or use vendored OpenSSL
   cargo build --features vendored-openssl
   ```

4. **Out of disk space**:
   ```bash
   # Check disk usage
   df -h target/
   
   # Clean build artifacts
   cargo clean
   
   # Use different target directory
   export CARGO_TARGET_DIR=/large-storage/target
   ```

5. **Network/registry issues**:
   ```bash
   # Use different registry
   export CARGO_REGISTRIES_CRATES_IO_INDEX="https://github.com/rust-lang/crates.io-index"
   
   # Use offline mode (if dependencies cached)
   cargo build --offline
   ```

### Build Performance Issues

```bash
# Use more CPU cores
export CARGO_BUILD_JOBS=$(nproc)

# Use faster linker (Linux)
sudo dnf install mold  # Fedora
cargo build --config 'target.x86_64-unknown-linux-gnu.linker="clang"' --config 'target.x86_64-unknown-linux-gnu.rustflags=["-C", "link-arg=-fuse-ld=mold"]'

# Enable incremental compilation
export CARGO_INCREMENTAL=1

# Use shared target directory for multiple projects
export CARGO_TARGET_DIR=~/.cargo/target-shared
```

### Debug Build Issues

```bash
# Build with verbose output
cargo build -v

# Build with specific log level
RUST_LOG=debug cargo build

# Check dependency tree
cargo tree

# Audit dependencies for security issues
cargo audit

# Check for outdated dependencies
cargo outdated
```

## Installation After Building

### Local Installation

```bash
# Install to ~/.cargo/bin (in PATH by default)
cargo install --path .

# Verify installation
bcvk --version

# Install specific binary
cargo install --bin bcvk --path .
```

### System Installation

```bash
# Install to system location
sudo cp target/release/bcvk /usr/local/bin/

# Make executable
sudo chmod +x /usr/local/bin/bcvk

# Verify system installation
which bcvk
bcvk --version
```

### Package Creation

```bash
# Create distributable package
cargo package

# Check package contents
cargo package --list

# Publish to crates.io (maintainers only)
cargo publish
```

## Development Environment Setup

### IDE Configuration

#### VS Code
Create `.vscode/settings.json`:
```json
{
    "rust-analyzer.cargo.buildScripts.enable": true,
    "rust-analyzer.checkOnSave.command": "clippy",
    "rust-analyzer.cargo.features": "all"
}
```

#### Vim/Neovim
Install rust.vim plugin and configure LSP with rust-analyzer.

### Git Hooks

Set up pre-commit hooks:

```bash
# Create pre-commit hook
cat > .git/hooks/pre-commit <<'EOF'
#!/bin/bash
set -e

echo "Running pre-commit checks..."

# Format check
if ! cargo fmt -- --check; then
    echo "Code needs formatting. Run: cargo fmt"
    exit 1
fi

# Clippy check
cargo clippy -- -D warnings

# Test check
cargo test

echo "All pre-commit checks passed!"
EOF

chmod +x .git/hooks/pre-commit
```

## Contributing to Development

### Setting Up for Contributions

```bash
# Fork the repository on GitHub, then:
git clone https://github.com/yourusername/bcvk.git
cd bcvk

# Add upstream remote
git remote add upstream https://github.com/cgwalters/bcvk.git

# Create feature branch
git checkout -b feature/my-improvement

# Make changes, then:
cargo fmt
cargo clippy
cargo test

# Commit and push
git commit -m "feat: add new feature"
git push origin feature/my-improvement
```

### Running Full Test Suite

```bash
# Complete test workflow
just fmt
just clippy
just test
just test-integration

# Or manually:
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo test --test integration
```

## Performance Optimization

### Profile-Guided Optimization (PGO)

```bash
# Build instrumented binary
RUSTFLAGS="-Cprofile-generate=/tmp/pgo-data" cargo build --release

# Run typical workloads
./target/release/bcvk ephemeral run test-image
./target/release/bcvk to-disk test-image /tmp/test.img

# Build optimized binary
RUSTFLAGS="-Cprofile-use=/tmp/pgo-data" cargo build --release
```

### Link-Time Optimization (LTO)

Add to `Cargo.toml`:
```toml
[profile.release]
lto = true
codegen-units = 1
panic = "abort"
```

### Size Optimization

For smaller binaries:
```toml
[profile.release]
opt-level = "s"    # Optimize for size
strip = true       # Strip symbols
lto = true
panic = "abort"
```

## Next Steps

After successfully building bcvk:

1. **Test your build** with the [testing guide](./testing.md)
2. **Contribute** following the [contributing guidelines](./contributing.md)
3. **Deploy** using the [installation guide](./installation.md)
4. **Learn** the [architecture](./architecture.md) for deeper understanding
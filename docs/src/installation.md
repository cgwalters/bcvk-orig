# Installation

Currently you need to build from source.

## Prerequisites

Before installing bcvk, ensure your system meets these requirements:

### Required Dependencies

1. [Rust](https://www.rust-lang.org/)

2. **Git**: Install from your distribution's package manager or [git-scm.com](https://git-scm.com/downloads)

3. **QEMU and KVM**: Install from your distribution or [qemu.org](https://www.qemu.org/download/)

4. **virtiofsd**: Usually included with QEMU, or available from your distribution

5. **Podman**: Install from [podman.io](https://podman.io/getting-started/installation)

### Optional Dependencies

6. **libvirt** (optional, for persistent VM features): Install from your distribution or [libvirt.org](https://libvirt.org/downloads.html)
   - Enable the libvirt daemon: `sudo systemctl enable --now libvirtd`
   - Add your user to the libvirt group: `sudo usermod -a -G libvirt $USER`

## Installation Methods

### Building from Source

Currently, the primary installation method is building from source:

1. **Clone the Repository**:
   ```bash
   git clone https://github.com/cgwalters/bcvk.git
   cd bcvk
   ```

2. **Build the Project**:
   ```bash
   cargo build --release
   ```
   
   The compiled binary will be available at `target/release/bcvk`.

3. **Install to PATH** (optional):
   ```bash
   # Copy to a directory in your PATH
   sudo cp target/release/bcvk /usr/local/bin/
   
   # Or create a symlink
   sudo ln -s $(pwd)/target/release/bcvk /usr/local/bin/bcvk
   
   # Or add the target/release directory to your PATH
   echo 'export PATH="$PATH:$(pwd)/target/release"' >> ~/.bashrc
   source ~/.bashrc
   ```

## Platform-Specific Notes

### Linux

- Ensure your user has access to KVM: check that `/dev/kvm` exists and your user can read it
- For libvirt features, make sure your user is in the `libvirt` group
- Some distributions may require additional SELinux configuration for virtualization

### macOS

- Not supported yet, use [podman-bootc](https://github.com/containers/podman-bootc/)

### Windows

- Not supported

## Next Steps

Once bcvk is installed, continue with the [Quick Start Guide](./quick-start.md) to learn how to use the tool effectively.
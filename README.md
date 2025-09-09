# A toolkit for virtualization and bootc

This project helps launch ephemeral VMs from bootc containers, and also create
disk images that can be imported into other virtualization frameworks.

## Installation

For now `git clone && cargo build --release`.

## Quick Start

### Running a bootc container as ephemeral VM 

This doesn't require any privileges, it's just a wrapper
for `podman`. It does require a virt stack (qemu, virtiofsd)
in the host environment.

```bash
# Basic ephemeral VM with rescue mode
./target/release/bootc-kit run-ephemeral --rm -ti quay.io/fedora/fedora-bootc:42 \
  --karg=systemd.unit=rescue.target \
  --karg=systemd.setenv=SYSTEMD_SULOGIN_FORCE=1

# VM with custom resources and SSH access
bootc-kit run-ephemeral --memory=4g --vcpus=4 --console quay.io/fedora/fedora-bootc:42
```

### Installing to disk
```bash
# Install bootc image to disk
bootc-kit run-install quay.io/fedora/fedora-bootc:42 /path/to/disk.img \
  --root-size=20G --filesystem=ext4

# Boot from installed disk
bootc-kit run-disk /path/to/disk.img --memory=2g
```

### Unified install-and-run workflow
```bash
# Install and run in a single command (container-based)
bootc-kit run-from-install --ssh-keygen --name myvm quay.io/fedora/fedora-bootc:42

# With custom resources and virtiofs mounts
bootc-kit run-from-install \
  --memory=4g --vcpus=4 \
  --virtiofs-mounts=/host/data:/data \
  --ssh-keygen --name myvm \
  quay.io/fedora/fedora-bootc:42

# Run in detached mode for background execution  
bootc-kit run-from-install --detach --ssh-keygen --name myvm \
  quay.io/fedora/fedora-bootc:42
```

### SSH access
```bash
# SSH into running VM
bootc-kit ssh <container-name>
```

### Image management
```bash
# List bootc images
bootc-kit images list

# List as JSON
bootc-kit images list --json
```

## Command Comparison

| Command | Purpose | Environment | Use Case |
|---------|---------|-------------|----------|
| `run-ephemeral` | Run container images as VMs | Container-based | Quick testing, development |
| `run-install` | Install bootc to disk images | Container-based | Create persistent disk images |
| `run-disk` | Boot from disk images | Host QEMU | Run existing disk images |
| `run-from-install` | Install + boot in unified workflow | Container-based | One-stop VM creation and execution |

### Key Benefits of `run-from-install`

- **Unified Workflow**: Combines installation and runtime in a single command
- **Container-Based**: Everything runs within containers, no direct host QEMU
- **SSH Integration**: Built-in SSH key generation and `bootc-kit ssh` support
- **Container Lifecycle**: Supports detached mode, naming, and cleanup options
- **Full Feature Parity**: Virtiofs mounts, additional disks, networking, etc.

### Example Workflow

```bash
# Start a persistent VM with SSH access
bootc-kit run-from-install --ssh-keygen --name myvm --detach \
  quay.io/fedora/fedora-bootc:42

# SSH into the running VM
bootc-kit ssh myvm

# Stop and remove the VM
podman stop myvm && podman rm myvm
```

## Goals

This project aims to implement
<https://gitlab.com/fedora/bootc/tracker/-/issues/2>.

Related projects and content:

- https://github.com/coreos/coreos-assembler/
- https://github.com/ublue-os/bluefin-lts/blob/main/Justfile

## Development

See docs/HACKING.md


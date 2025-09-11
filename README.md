# bcvk - bootc virtualization kit

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
# VM with custom resources
bcvk run-ephemeral -d --rm -K --name mytestvm quay.io/fedora/fedora-bootc:42
bcvk ssh mytestvm
```

### Creating a persistent bootable disk image from a container image
```bash
# Install bootc image to disk
bcvk run-install quay.io/centos-bootc/centos-bootc:stream10 /path/to/disk.img
```

### Image management
```bash
# List bootc images
bcvk images list

# List as JSON
bcvk images list --json
```

## Command Comparison

| Command | Purpose | Environment | Use Case |
|---------|---------|-------------|----------|
| `run-ephemeral` | Run container images as VMs | Container-based | Quick testing, development |
| `run-install` | Install bootc to disk images | Container-based | Create persistent disk images |
| `run-disk` | Boot from disk images | Host QEMU | Run existing disk images |

## Goals

This project aims to implement part of
<https://gitlab.com/fedora/bootc/tracker/-/issues/2>.

Basically it will be "bootc virtualization kit", and help users
run bootable containers as virtual machines.

Related projects and content:

- https://github.com/coreos/coreos-assembler/
- https://github.com/ublue-os/bluefin-lts/blob/main/Justfile

## Development

See docs/HACKING.md


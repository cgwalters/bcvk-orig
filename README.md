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
bcvk run-ephemeral -d --rm -K --name mytestvm quay.io/fedora/fedora-bootc:42
bcvk ssh mytestvm
```

### Creating a persistent bootable disk image from a container image
```bash
# Install bootc image to disk
bcvk run-install quay.io/centos-bootc/centos-bootc:stream10 /path/to/disk.img
```

### Image management

There's a convenient helper function which filters by all container images
with the `containers.bootc=1` label: `bcvk images list`

## Goals

This project aims to implement part of
<https://gitlab.com/fedora/bootc/tracker/-/issues/2>.

Basically it will be "bootc virtualization kit", and help users
run bootable containers as virtual machines.

Related projects and content:

- https://github.com/coreos/coreos-assembler/
- https://github.com/ublue-os/bluefin-lts/blob/main/Justfile

## Development

See [docs/HACKING.md](docs/HACKING.md).



# Quick Start

This guide will get you up and running with bcvk quickly. We'll cover the most common use cases and essential commands to help you start working with bootc containers as virtual machines.

## Prerequisites

Before starting, make sure you have:
- bcvk installed (see [Installation Guide](./installation.md))
- A working container runtime (podman)
- Virtualization support (QEMU/KVM)
- At least one bootc container image available

## Your First VM

The fastest way to get started is launching an ephemeral VM with automatic SSH:

```bash
bcvk ephemeral run-ssh quay.io/fedora/fedora-bootc:42
```

This single command will start a virtual machine inside a podman
container, mounting the container's root via virtiofsd, using the
kernel+initramfs from the image. Then it will automatically `ssh`
to the target system. When the `ssh` client terminates (e.g. by
running `exit` in the remote shell), the pod (and virtual machine)
also terminate.

## Basic Workflows

### 1. Ephemeral VMs for Testing

Ephemeral VMs are perfect for quick testing and development:

```bash
# Start a VM in the background
bcvk ephemeral run -d --rm -K --name mytestvm quay.io/fedora/fedora-bootc:42

# SSH into the running VM
bcvk ephemeral ssh mytestvm

# The VM will be automatically cleaned up when stopped
```

Key options:
- `-d` or `--detach`: Run in the background
- `--rm`: Remove the VM when it stops
- `-K`: Generate and inject SSH keys automatically
- `--name`: Give the VM a specific name

### 2. Creating Disk Images

Convert bootc containers to disk images for deployment:

```bash
# Create a raw disk image
bcvk to-disk quay.io/centos-bootc/centos-bootc:stream10 /path/to/disk.img

# Create a qcow2 disk image (more compact)
bcvk to-disk --format qcow2 quay.io/fedora/fedora-bootc:42 /path/to/fedora.qcow2

# Create with specific disk size
bcvk to-disk --size 20G quay.io/fedora/fedora-bootc:42 /path/to/large-disk.img
```

These disk images can be:
- Deployed to cloud platforms
- Written to physical drives
- Used in other virtualization platforms

### 3. Persistent VMs with libvirt

For long-running VMs, use libvirt integration:

```bash
# Create and start a persistent VM
bcvk libvirt run --name my-server quay.io/fedora/fedora-bootc:42

# SSH into the VM
bcvk libvirt ssh my-server

# Stop the VM (but keep it for later)
bcvk libvirt stop my-server

# Start it again
bcvk libvirt start my-server

# List all bootc VMs
bcvk libvirt list

# Remove the VM completely
bcvk libvirt rm my-server
```

### 4. Image Management

Discover and manage bootc container images:

```bash
# List all bootc images (those with containers.bootc=1 label)
bcvk images list

# This helps you find available bootc containers on your system
```

## Common Command Patterns

### Resource Configuration

Customize VM resources when creating them:

```bash
# Custom memory and CPU for ephemeral VM
bcvk ephemeral run --memory 4096 --cpus 4 --name bigvm quay.io/fedora/fedora-bootc:42

# Custom resources for libvirt VM
bcvk libvirt run --name webserver --memory 8192 --cpus 8 --disk-size 50G quay.io/centos-bootc/centos-bootc:stream10
```

## Next Steps

Now that you're familiar with the basics:

1. **Complete Command Reference**: See [Command Reference](./man/bcvk.md) for all available options and examples
2. **Compare Workflows**: Check [Workflow Comparison](./workflow-comparison.md) to understand when to use bcvk vs alternatives
3. **Development Setup**: See [Building from Source](./building.md) if you want to contribute or customize bcvk
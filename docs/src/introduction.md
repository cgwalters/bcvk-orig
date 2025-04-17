# Introduction

**bcvk** (bootc virtualization kit) is a powerful tool designed to simplify working with bootc containers by launching them as virtual machines and creating bootable disk images. It bridges the gap between container workflows and virtualization, making it easy to test, deploy, and manage bootc-enabled container images.

## What is bcvk?

bcvk helps you work with bootc containers in several key ways:

- **Ephemeral VMs**: Launch temporary virtual machines directly from bootc container images without requiring root privileges
- **Persistent Disk Images**: Create bootable disk images from container images that can be deployed to physical or virtual infrastructure
- **libvirt Integration**: Comprehensive integration with libvirt for managing persistent VMs with full lifecycle management
- **Image Management**: Tools for discovering and managing bootc container images

## Key Features

### Ephemeral Virtual Machines
Run bootc containers as temporary VMs for testing and development. These VMs are launched using QEMU and can be easily created, accessed via SSH, and automatically cleaned up when no longer needed.

### Disk Image Creation
Convert bootc container images into bootable disk images (`.img`, `.qcow2`, `.raw`) that can be:
- Deployed to cloud platforms
- Installed on physical hardware
- Imported into various virtualization frameworks
- Used as base images for further customization

### libvirt Integration
Full-featured integration with libvirt allowing you to:
- Create persistent VMs from bootc containers
- Manage VM lifecycle (start, stop, restart, remove)
- Configure VM resources (CPU, memory, disk, networking)
- Upload and manage disk images in libvirt storage pools
- SSH access and port forwarding

### Container Image Management
Built-in tools to discover and list bootc container images, filtering by the `containers.bootc=1` label to identify bootc-compatible images.

## Use Cases

### Development and Testing
- Quickly spin up VMs from container images for testing
- Iterate on container builds and immediately test in VM environments
- SSH into running VMs for debugging and development

### Infrastructure Deployment
- Create disk images for cloud deployment (AWS, GCP, Azure)
- Generate images for bare-metal installations
- Prepare base images for infrastructure as code workflows

### CI/CD Integration
- Automate testing of bootc container images in VM environments
- Create deployment artifacts (disk images) as part of build pipelines
- Validate container-to-VM conversion processes

### Hybrid Container/VM Workflows
- Bridge container development with traditional VM operations
- Leverage existing libvirt infrastructure for container-based workloads
- Provide VM interfaces for applications designed for container deployment

## Why bcvk?

Traditional approaches to working with bootc containers often require complex setup, root privileges, or manual configuration of virtualization infrastructure. bcvk simplifies these workflows by:

1. **No Root Required**: Ephemeral VMs run using podman without requiring root privileges
2. **Unified Interface**: Single tool for multiple workflows (ephemeral VMs, disk creation, libvirt management)
3. **Quick Iteration**: Fast container-to-VM conversion for rapid development cycles
4. **Production Ready**: Create production-ready disk images and manage persistent VMs
5. **Standard Tools**: Built on industry-standard tools (QEMU, libvirt, podman) for reliability and compatibility

## Getting Started

For detailed setup instructions and your first VM, see the [Quick Start Guide](./quick-start.md).
# Disk Images

bcvk provides powerful capabilities for creating bootable disk images from bootc container images. These disk images can be deployed to various platforms, written to physical media, or used in different virtualization environments.

## Overview

Disk image creation converts bootc container images into bootable disk images that contain:
- Complete operating system from the container
- Bootloader configuration
- Partitioning scheme
- File system structure
- All software and configurations from the container

## Supported Image Formats

### Raw Images (.img, .raw)
Uncompressed disk images that can be directly written to storage devices.

```bash
# Create raw disk image
bcvk to-disk quay.io/fedora/fedora-bootc:42 /path/to/fedora.img

# Specify raw format explicitly
bcvk to-disk --format raw quay.io/fedora/fedora-bootc:42 /path/to/fedora.raw
```

**Use cases:**
- Writing to USB drives or SD cards
- Cloud deployment (AWS, GCP, Azure)
- Bare-metal installation
- Simple virtualization setups

### QCOW2 Images (.qcow2)
Compressed, copy-on-write disk images commonly used with QEMU/KVM.

```bash
# Create QCOW2 image
bcvk to-disk --format qcow2 quay.io/fedora/fedora-bootc:42 /path/to/fedora.qcow2
```

**Benefits:**
- Smaller file size due to compression
- Copy-on-write capabilities
- Snapshot support
- Sparse allocation (only allocated space is used)

**Use cases:**
- QEMU/KVM virtualization
- libvirt-managed VMs
- Development and testing environments

### VHD/VHDX Images
Virtual Hard Disk format for Microsoft virtualization platforms.

```bash
# Create VHD image
bcvk to-disk --format vhd quay.io/fedora/fedora-bootc:42 /path/to/fedora.vhd
```

**Use cases:**
- Hyper-V virtualization
- Azure cloud deployment
- Windows-based virtualization environments

### VMDK Images
VMware Virtual Machine Disk format.

```bash
# Create VMDK image
bcvk to-disk --format vmdk quay.io/fedora/fedora-bootc:42 /path/to/fedora.vmdk
```

**Use cases:**
- VMware vSphere/ESXi
- VMware Workstation/Fusion
- vCloud environments

## Image Configuration Options

### Disk Size

Configure the total disk size:

```bash
# 50GB disk
bcvk to-disk --size 50G quay.io/fedora/fedora-bootc:42 /path/to/large.img

# 10GB minimal disk
bcvk to-disk --size 10G quay.io/centos-bootc/centos-bootc:stream10 /path/to/minimal.img

# Default size (usually 20GB)
bcvk to-disk quay.io/fedora/fedora-bootc:42 /path/to/default.img
```

Size formats supported:
- `G` or `GB`: Gigabytes
- `M` or `MB`: Megabytes
- `T` or `TB`: Terabytes

### File System Options

Select the root filesystem type:

```bash
# XFS filesystem (default on many distros)
bcvk to-disk --filesystem xfs quay.io/fedora/fedora-bootc:42 /path/to/xfs.img

# ext4 filesystem
bcvk to-disk --filesystem ext4 quay.io/fedora/fedora-bootc:42 /path/to/ext4.img

# Btrfs filesystem
bcvk to-disk --filesystem btrfs quay.io/fedora/fedora-bootc:42 /path/to/btrfs.img
```

### Partitioning Schemes

Choose partitioning layout:

```bash
# UEFI/GPT partitioning (modern, recommended)
bcvk to-disk --partition gpt quay.io/fedora/fedora-bootc:42 /path/to/uefi.img

# Legacy BIOS/MBR partitioning
bcvk to-disk --partition mbr quay.io/fedora/fedora-bootc:42 /path/to/bios.img

# Automatic (based on image defaults)
bcvk to-disk quay.io/fedora/fedora-bootc:42 /path/to/auto.img
```

## Use Cases and Deployment Scenarios

### Cloud Deployment

#### AWS EC2
```bash
# Create image suitable for AWS
bcvk to-disk --format raw --size 30G \
  quay.io/fedora/fedora-bootc:42 /tmp/aws-fedora.img

# Convert to AMI and upload (using AWS CLI)
aws ec2 import-image --description "Fedora bootc" \
  --disk-containers file://aws-disk-config.json
```

#### Google Cloud Platform
```bash
# Create image for GCP
bcvk to-disk --format raw --size 20G \
  quay.io/fedora/fedora-bootc:42 /tmp/gcp-fedora.img

# Upload to GCP (using gcloud CLI)
gcloud compute images create fedora-bootc \
  --source-disk-zone=us-central1-a \
  --source-disk=fedora-disk
```

#### Microsoft Azure
```bash
# Create VHD for Azure
bcvk to-disk --format vhd --size 30G \
  quay.io/centos-bootc/centos-bootc:stream10 /tmp/azure-centos.vhd

# Upload to Azure (using Azure CLI)
az image create --resource-group myRG --name centos-bootc \
  --source /tmp/azure-centos.vhd
```

### Bare-Metal Installation

#### USB/SD Card Creation
```bash
# Create image for USB drive
bcvk to-disk --size 16G quay.io/fedora/fedora-bootc:42 /tmp/usb-install.img

# Write to USB device (be careful with device selection!)
sudo dd if=/tmp/usb-install.img of=/dev/sdX bs=4M status=progress
```

#### ISO Creation (if supported)
```bash
# Create ISO for CD/DVD burning
bcvk to-disk --format iso quay.io/fedora/fedora-bootc:42 /tmp/fedora-bootc.iso

# Burn to optical media
wodim -v dev=/dev/sr0 /tmp/fedora-bootc.iso
```

### Virtualization Platforms

#### QEMU/KVM
```bash
# Create QCOW2 for local virtualization
bcvk to-disk --format qcow2 --size 25G \
  quay.io/fedora/fedora-bootc:42 /var/lib/libvirt/images/fedora-bootc.qcow2

# Run with QEMU directly
qemu-system-x86_64 -hda /var/lib/libvirt/images/fedora-bootc.qcow2 \
  -m 2048 -enable-kvm
```

#### VMware
```bash
# Create VMDK for VMware
bcvk to-disk --format vmdk --size 30G \
  quay.io/centos-bootc/centos-bootc:stream10 /tmp/centos-bootc.vmdk

# Import into VMware Workstation or vSphere
```

#### VirtualBox
```bash
# Create raw image, then convert to VDI
bcvk to-disk --format raw --size 25G \
  quay.io/fedora/fedora-bootc:42 /tmp/fedora.img

# Convert to VirtualBox format
VBoxManage convertfromraw /tmp/fedora.img /tmp/fedora.vdi
```

## Advanced Configuration

### Custom Bootloader Options

```bash
# GRUB2 bootloader (most common)
bcvk to-disk --bootloader grub2 quay.io/fedora/fedora-bootc:42 /path/to/grub.img

# systemd-boot (UEFI only)
bcvk to-disk --bootloader systemd-boot quay.io/fedora/fedora-bootc:42 /path/to/systemd.img
```

### Security Features

```bash
# Enable LUKS disk encryption
bcvk to-disk --encrypt-disk --passphrase-file /tmp/password \
  quay.io/fedora/fedora-bootc:42 /path/to/encrypted.img

# Enable Secure Boot
bcvk to-disk --secure-boot quay.io/fedora/fedora-bootc:42 /path/to/secure.img
```

### Multi-Architecture Support

```bash
# Create ARM64 image
bcvk to-disk --arch arm64 quay.io/fedora/fedora-bootc:42 /path/to/arm64.img

# Create x86_64 image (default)
bcvk to-disk --arch amd64 quay.io/fedora/fedora-bootc:42 /path/to/amd64.img
```

## Performance Considerations

### Image Size Optimization

```bash
# Minimal image size for testing
bcvk to-disk --size 8G --filesystem ext4 \
  quay.io/fedora/fedora-bootc:42 /tmp/minimal.img

# Production size with room for growth
bcvk to-disk --size 50G --filesystem xfs \
  quay.io/fedora/fedora-bootc:42 /tmp/production.img
```

### Format Selection Guidelines

- **Raw images**: Best compatibility, larger size
- **QCOW2**: Good compression, KVM-specific features
- **VHD/VHDX**: Best for Microsoft environments
- **VMDK**: Best for VMware environments

### Storage Considerations

```bash
# Check available disk space before creation
df -h /tmp

# Create on fastest available storage
bcvk to-disk quay.io/fedora/fedora-bootc:42 /fast-ssd/image.img

# Use temporary directory with sufficient space
export TMPDIR=/large-storage/tmp
bcvk to-disk quay.io/fedora/fedora-bootc:42 /path/to/image.img
```

## Image Validation and Testing

### Image Verification

```bash
# Check image properties
qemu-img info /path/to/image.qcow2

# Test image boot with QEMU
qemu-system-x86_64 -hda /path/to/image.img -m 2048 -enable-kvm

# Verify filesystem integrity
fsck.ext4 -n /path/to/image.img  # For ext4 images
```

### Automated Testing

```bash
#!/bin/bash
# Test script for image validation
IMAGE_PATH="/tmp/test-image.qcow2"

# Create test image
bcvk to-disk --format qcow2 quay.io/fedora/fedora-bootc:42 $IMAGE_PATH

# Boot test with timeout
timeout 300 qemu-system-x86_64 \
  -hda $IMAGE_PATH \
  -m 2048 \
  -enable-kvm \
  -nographic \
  -serial stdio

echo "Image test completed"
```

## Troubleshooting

### Common Issues

1. **Insufficient disk space**:
   ```bash
   # Check available space
   df -h /tmp
   
   # Use different output directory
   bcvk to-disk image /large-storage/output.img
   ```

2. **Unsupported format**:
   ```bash
   # Check supported formats
   bcvk to-disk --help | grep format
   
   # Use supported format
   bcvk to-disk --format qcow2 image output.qcow2
   ```

3. **Image won't boot**:
   ```bash
   # Verify image integrity
   qemu-img check output.qcow2
   
   # Check bootloader installation
   qemu-system-x86_64 -hda output.img -m 2048
   ```

4. **Size too small**:
   ```bash
   # Increase image size
   bcvk to-disk --size 30G image larger-output.img
   
   # Check container size requirements
   podman images --format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}"
   ```

### Debug Information

```bash
# Verbose output for debugging
bcvk -v to-disk quay.io/fedora/fedora-bootc:42 /tmp/debug.img

# Check temporary files (if process fails)
ls -la /tmp/bcvk-*

# Monitor disk usage during creation
watch -n 1 df -h /tmp
```

## Best Practices

### Image Creation Workflow

1. **Test with small images first**
2. **Verify container functionality** before image creation
3. **Use appropriate formats** for target platform
4. **Plan for adequate disk space**
5. **Test images** before deployment

### Security Best Practices

1. **Use secure base images**
2. **Enable disk encryption** for sensitive workloads
3. **Verify image checksums** before deployment
4. **Use secure storage** for image files

### Performance Best Practices

1. **Choose optimal filesystem** for workload
2. **Size images appropriately** (not too small, not wastefully large)
3. **Use faster storage** for image creation
4. **Consider compression trade-offs**

## Next Steps

- Learn about [creating disk images](./to-disk.md) with detailed command options
- Explore [libvirt integration](./libvirt-integration.md) for VM management
- Understand [storage management](./storage-management.md) concepts
- Read about [architecture](./architecture.md) to understand the conversion process
# Image Management

bcvk provides tools for discovering, listing, and managing bootc container images. This guide covers how to work with bootc container images using the `bcvk images` command and related functionality.

## Overview

The image management features help you:
- Discover bootc-compatible container images
- List available bootc images on your system
- Filter and search through bootc images
- Understand image properties and metadata
- Manage local bootc image cache

## bootc Image Identification

bcvk identifies bootc container images using the `containers.bootc=1` label. This label indicates that the container image is designed to be used as a bootable system image.

### Checking if an Image is bootc-Compatible

```bash
# Check image labels
podman inspect quay.io/fedora/fedora-bootc:42 | grep -i bootc

# Look for the bootc label specifically
podman inspect quay.io/fedora/fedora-bootc:42 --format '{{.Config.Labels}}'
```

## The images Command

### Basic Usage

The `bcvk images` command provides subcommands for working with bootc images:

```bash
# List all bootc images
bcvk images list

# Get help on images commands
bcvk images --help
```

## Listing bootc Images

### Basic Listing

```bash
# List all local bootc images
bcvk images list

# Example output:
# REPOSITORY                           TAG      IMAGE ID      CREATED       SIZE
# quay.io/fedora/fedora-bootc         42       a1b2c3d4e5f6  2 days ago    1.2GB
# quay.io/centos-bootc/centos-bootc   stream10 f6e5d4c3b2a1  1 week ago    890MB
# localhost/my-bootc-app              latest   1a2b3c4d5e6f  1 hour ago    1.5GB
```

### Filtering Options

```bash
# List images with specific repository
bcvk images list --filter repository=fedora

# List images by tag
bcvk images list --filter tag=42

# List recently created images
bcvk images list --filter since=24h

# List images larger than specific size
bcvk images list --filter size=>1GB
```

### Output Formats

```bash
# Default table format
bcvk images list

# JSON output for scripting
bcvk images list --format json

# Custom format
bcvk images list --format "{{.Repository}}\t{{.Tag}}\t{{.Size}}"

# Only show image IDs
bcvk images list --format "{{.ID}}"
```

## Image Discovery

### Finding Public bootc Images

```bash
# Search for Fedora bootc images
podman search fedora-bootc

# Search for CentOS bootc images  
podman search centos-bootc

# Search in specific registries
podman search --limit 20 registry.redhat.io/ubi9-bootc
```

### Popular bootc Images

Common publicly available bootc images:

```bash
# Fedora bootc images
quay.io/fedora/fedora-bootc:42
quay.io/fedora/fedora-bootc:41
quay.io/fedora/fedora-bootc:rawhide

# CentOS Stream bootc images
quay.io/centos-bootc/centos-bootc:stream10
quay.io/centos-bootc/centos-bootc:stream9

# Red Hat Enterprise Linux bootc images
registry.redhat.io/rhel9/rhel-bootc:latest
```

### Pulling bootc Images

```bash
# Pull specific bootc image
podman pull quay.io/fedora/fedora-bootc:42

# Pull all tags of an image
podman pull --all-tags quay.io/fedora/fedora-bootc

# Pull for specific architecture
podman pull --arch arm64 quay.io/fedora/fedora-bootc:42
```

## Image Information and Inspection

### Detailed Image Information

```bash
# Get detailed information about a bootc image
bcvk images info quay.io/fedora/fedora-bootc:42

# Show image history
podman history quay.io/fedora/fedora-bootc:42

# Inspect image configuration
podman inspect quay.io/fedora/fedora-bootc:42
```

### Understanding Image Metadata

```bash
# Check bootc-specific labels
podman inspect quay.io/fedora/fedora-bootc:42 \
  --format '{{.Config.Labels.containers.bootc}}'

# Check OS information
podman inspect quay.io/fedora/fedora-bootc:42 \
  --format '{{.Os}} {{.Architecture}}'

# Check creation date
podman inspect quay.io/fedora/fedora-bootc:42 \
  --format '{{.Created}}'

# Check image size
podman inspect quay.io/fedora/fedora-bootc:42 \
  --format '{{.Size}}'
```

### Image Layers and Components

```bash
# Show image layers
podman history --no-trunc quay.io/fedora/fedora-bootc:42

# Dive into image filesystem (requires dive tool)
dive quay.io/fedora/fedora-bootc:42

# Export image filesystem for inspection
podman save quay.io/fedora/fedora-bootc:42 | tar -tv
```

## Local Image Management

### Cleaning Up Images

```bash
# Remove unused bootc images
bcvk images prune

# Remove specific image
podman rmi quay.io/fedora/fedora-bootc:42

# Remove all bootc images (use with caution)
bcvk images list --format "{{.Repository}}:{{.Tag}}" | xargs podman rmi

# Clean up all unused images and containers
podman system prune -a
```

### Image Storage Information

```bash
# Check local storage usage
podman system df

# Detailed storage breakdown
podman system df -v

# Check storage location
podman info --format '{{.Store.GraphRoot}}'

# List storage by image
du -sh ~/.local/share/containers/storage/overlay/*
```

### Tagging and Renaming

```bash
# Tag a bootc image with new name
podman tag quay.io/fedora/fedora-bootc:42 localhost/my-fedora:latest

# Create multiple tags
podman tag quay.io/fedora/fedora-bootc:42 localhost/fedora-base:v1.0
podman tag quay.io/fedora/fedora-bootc:42 localhost/fedora-base:stable

# Remove tag (not the image)
podman rmi localhost/my-fedora:latest
```

## Working with Private Registries

### Authentication

```bash
# Login to private registry
podman login registry.company.com

# Login with username/password
podman login -u username -p password registry.company.com

# Login with token
echo $TOKEN | podman login --password-stdin registry.company.com
```

### Pulling from Private Registries

```bash
# Pull from authenticated registry
podman pull registry.company.com/bootc/app:latest

# List private bootc images
bcvk images list --filter repository=registry.company.com

# Check if image is bootc-compatible
podman inspect registry.company.com/bootc/app:latest \
  --format '{{.Config.Labels.containers.bootc}}'
```

## Building Custom bootc Images

### Creating a bootc Containerfile

```dockerfile
# Example Containerfile for bootc image
FROM quay.io/fedora/fedora-bootc:42

# Add bootc label (should be inherited, but explicit is good)
LABEL containers.bootc=1

# Install additional packages
RUN dnf install -y \
    httpd \
    nginx \
    git && \
    dnf clean all

# Configure services
RUN systemctl enable httpd nginx

# Add custom configuration
COPY config/httpd.conf /etc/httpd/conf/
COPY web-content/ /var/www/html/

# Create users
RUN useradd -m -s /bin/bash webuser

# Set permissions
RUN chown -R webuser:webuser /var/www/html
```

### Building bootc Images

```bash
# Build custom bootc image
podman build -t localhost/my-web-bootc:latest .

# Build with specific architecture
podman build --arch arm64 -t localhost/my-web-bootc:arm64 .

# Build with build arguments
podman build --build-arg VERSION=1.0 -t localhost/my-app:v1.0 .

# Verify the image is bootc-compatible
bcvk images list --filter repository=localhost/my-web-bootc
```

### Testing Custom Images

```bash
# Test the custom image with ephemeral VM
bcvk ephemeral run-ssh localhost/my-web-bootc:latest

# Create disk image from custom image
bcvk to-disk localhost/my-web-bootc:latest /tmp/custom-web.img

# Test with libvirt
bcvk libvirt run --name test-custom localhost/my-web-bootc:latest
```

## Image Registry Operations

### Pushing Images to Registries

```bash
# Tag for registry
podman tag localhost/my-bootc:latest registry.company.com/bootc/my-app:v1.0

# Push to registry
podman push registry.company.com/bootc/my-app:v1.0

# Push all tags
podman push --all-tags registry.company.com/bootc/my-app
```

### Registry Management

```bash
# List configured registries
podman info --format '{{.Registries}}'

# Add registry to configuration
# Edit ~/.config/containers/registries.conf

# Example registries.conf snippet:
[[registry]]
location = "registry.company.com"
insecure = false
blocked = false
```

## Automation and Scripting

### Scripting with Image Lists

```bash
#!/bin/bash
# Update all bootc images script

echo "Updating all bootc images..."

# Get list of bootc images
IMAGES=$(bcvk images list --format "{{.Repository}}:{{.Tag}}")

for image in $IMAGES; do
    echo "Updating $image..."
    podman pull "$image" || echo "Failed to update $image"
done

echo "Update complete"
```

### Image Validation Script

```bash
#!/bin/bash
# Validate bootc images script

validate_bootc_image() {
    local image="$1"
    
    # Check if image exists
    if ! podman inspect "$image" >/dev/null 2>&1; then
        echo "ERROR: Image $image not found"
        return 1
    fi
    
    # Check for bootc label
    local bootc_label=$(podman inspect "$image" \
        --format '{{.Config.Labels.containers.bootc}}' 2>/dev/null)
    
    if [ "$bootc_label" != "1" ]; then
        echo "ERROR: Image $image is not bootc-compatible"
        return 1
    fi
    
    echo "OK: Image $image is valid bootc image"
    return 0
}

# Validate all local bootc images
bcvk images list --format "{{.Repository}}:{{.Tag}}" | while read image; do
    validate_bootc_image "$image"
done
```

### CI/CD Integration

```yaml
# GitHub Actions example for image management
name: Bootc Image Management

on:
  schedule:
    - cron: '0 2 * * *'  # Daily at 2 AM
  workflow_dispatch:

jobs:
  update-images:
    runs-on: ubuntu-latest
    steps:
      - name: Update base images
        run: |
          # Update common bootc images
          podman pull quay.io/fedora/fedora-bootc:42
          podman pull quay.io/centos-bootc/centos-bootc:stream10
          
      - name: List updated images
        run: |
          bcvk images list
          
      - name: Test images
        run: |
          # Quick validation that images work
          bcvk ephemeral run -d --rm --name test-fedora \
            quay.io/fedora/fedora-bootc:42
          sleep 30
          podman stop test-fedora || true
```

## Troubleshooting Image Issues

### Common Problems

1. **Image not showing in bcvk images list**:
   ```bash
   # Check if image has bootc label
   podman inspect image:tag --format '{{.Config.Labels.containers.bootc}}'
   
   # Add bootc label if missing
   podman tag image:tag localhost/fixed:tag
   # Rebuild with proper Containerfile
   ```

2. **Authentication failures**:
   ```bash
   # Check login status
   podman login --get-login registry.company.com
   
   # Re-authenticate
   podman logout registry.company.com
   podman login registry.company.com
   ```

3. **Storage space issues**:
   ```bash
   # Check available space
   df -h ~/.local/share/containers
   
   # Clean up unused images
   podman system prune -a
   
   # Move storage location if needed
   # Edit ~/.config/containers/storage.conf
   ```

4. **Image pull failures**:
   ```bash
   # Try pulling with different transport
   podman pull docker://quay.io/fedora/fedora-bootc:42
   
   # Check registry connectivity
   curl -I https://quay.io/v2/
   
   # Use different registry mirror
   podman pull registry.fedoraproject.org/fedora-bootc:42
   ```

### Debug Information

```bash
# Get detailed information about image operations
podman --log-level debug pull quay.io/fedora/fedora-bootc:42

# Check podman configuration
podman info

# Verify storage integrity
podman system check

# Check for corrupted images
podman images --all --no-trunc
```

## Best Practices

### Image Management Workflow

1. **Regular Updates**: Keep base images updated
2. **Cleanup**: Regularly remove unused images
3. **Validation**: Verify images are bootc-compatible
4. **Documentation**: Document custom image purposes
5. **Testing**: Test images before production use

### Security Best Practices

1. **Use trusted registries** for base images
2. **Verify image signatures** when available
3. **Scan images for vulnerabilities**
4. **Use minimal base images** when possible
5. **Keep credentials secure**

### Performance Best Practices

1. **Use local registry mirrors** for frequently used images
2. **Layer caching**: Optimize Containerfile for layer reuse
3. **Image compression**: Use appropriate formats for storage
4. **Storage location**: Use fast storage for image operations

## Next Steps

- Learn about [ephemeral VMs](./ephemeral-vms.md) to test your images
- Explore [disk image creation](./to-disk.md) for deployment
- Understand [libvirt integration](./libvirt-integration.md) for persistent VMs
- Read about [building custom images](./building.md) for development workflows
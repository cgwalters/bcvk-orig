#!/bin/bash
set -euo pipefail
# Allow some commands to fail without stopping the script
set +e
# Setup container storage configuration for additional image store
# This allows bootc to access images from the mounted host storage

echo "=== Setting up container storage configuration ==="
echo "Checking for host storage mount:"
ls -la /run/ | grep virtiofs || echo "No virtiofs mounts found"
ls -la /run/virtiofs-mnt-hoststorage/ 2>/dev/null | head -3 || echo "Cannot access /run/virtiofs-mnt-hoststorage contents"

echo "Setting up container storage for additional image store"
echo "Found host storage mount at /run/virtiofs-mnt-hoststorage"
ls -la /run/virtiofs-mnt-hoststorage/ | head -5 || echo "Cannot list contents"

mkdir -p /etc/containers
# Mount tmpfs for /var/lib/containers to avoid overlay-on-overlay issues  
mkdir -p /var/lib/containers
mount -t tmpfs tmpfs /var/lib/containers
mkdir -p /var/lib/containers/storage

# Copy default storage.conf and modify it
echo "Looking for storage.conf:"
ls -la /usr/share/containers/storage.conf || echo "Not found at expected location"
# Just try to copy it regardless of the test result
cp /usr/share/containers/storage.conf /etc/containers/storage.conf 2>/dev/null && echo "Copied storage.conf successfully" || echo "Failed to copy storage.conf"
echo "Checking if /etc/containers/storage.conf exists after copy:"
ls -la /etc/containers/storage.conf 2>/dev/null || echo "File does not exist or cannot access"

echo "Proceeding with storage.conf modification"

# Check what sections exist in the file
echo "=== Original storage.conf sections ==="
grep '^\[' /etc/containers/storage.conf || true

if grep -q '^\[storage\.options\.overlay\]' /etc/containers/storage.conf; then
    # Check if additionalimagestores already exists
    if grep -q '^additionalimagestores = \[' /etc/containers/storage.conf; then
        echo "Found existing additionalimagestores, appending to it"
        # Add our path to the existing array
        sed -i '/^additionalimagestores = \[/a\
"/run/virtiofs-mnt-hoststorage",' /etc/containers/storage.conf
    else
        echo "Adding new storage.options section before overlay"
        # Add new section before overlay section
        sed -i '/^\[storage\.options\.overlay\]/i\
[storage.options]\
additionalimagestores = ["/run/virtiofs-mnt-hoststorage"]\
' /etc/containers/storage.conf
    fi
    # Also ensure fuse-overlayfs is used as mount_program if available
    if command -v fuse-overlayfs >/dev/null 2>&1; then
        if ! grep -q '^mount_program = ' /etc/containers/storage.conf; then
            echo "Adding fuse-overlayfs mount_program to overlay section"
            sed -i '/^\[storage\.options\.overlay\]/a\
mount_program = "/usr/bin/fuse-overlayfs"' /etc/containers/storage.conf
        fi
    fi
else
    echo "No overlay section found, adding storage.options at end"
    echo '[storage.options]' >> /etc/containers/storage.conf
    echo 'additionalimagestores = ["/run/virtiofs-mnt-hoststorage"]' >> /etc/containers/storage.conf
    if command -v fuse-overlayfs >/dev/null 2>&1; then
        echo '[storage.options.overlay]' >> /etc/containers/storage.conf
        echo 'mount_program = "/usr/bin/fuse-overlayfs"' >> /etc/containers/storage.conf
    fi
fi

echo "=== Modified storage.conf ==="
grep -B1 -A3 'additionalimagestores' /etc/containers/storage.conf || echo "additionalimagestores not found"

echo "=== Testing podman with storage configuration ==="
# Test that podman can see images from additional store
echo "Available images from host storage:"
podman images | head -10 || true
echo "Looking for fedora-bootc:42:"
podman images | grep 'fedora-bootc.*42' || echo "fedora-bootc:42 not found in podman images"

# Test inspect of the specific image
echo "Testing podman inspect of target image:"
podman image inspect quay.io/centos-bootc/centos-bootc:stream10 >/dev/null && echo "SUCCESS: Can inspect target image" || echo "FAIL: Cannot inspect target image"

echo "Container storage configuration completed"
# NAME

bcvk-libvirt-upload - Upload bootc disk images to libvirt with metadata annotations

# SYNOPSIS

**bcvk libvirt upload** [*OPTIONS*]

# DESCRIPTION

Upload bootc disk images to libvirt with metadata annotations

# OPTIONS

<!-- BEGIN GENERATED OPTIONS -->
**SOURCE_IMAGE**

    Container image to install and upload

    This argument is required.

**--volume-name**=*VOLUME_NAME*

    Name for the libvirt volume (defaults to sanitized image name)

**--pool**=*POOL*

    Libvirt storage pool name

    Default: default

**--disk-size**=*DISK_SIZE*

    Size of the disk image (e.g., '20G', '10240M'). If not specified, uses the actual size of the created disk

**--filesystem**=*FILESYSTEM*

    Root filesystem type (e.g. ext4, xfs, btrfs)

**--root-size**=*ROOT_SIZE*

    Root filesystem size (e.g., '10G', '5120M')

**--storage-path**=*STORAGE_PATH*

    Path to host container storage (auto-detected if not specified)

**--memory**=*MEMORY*

    Memory size for installation VM (e.g. 2G, 1024M)

    Default: 2048

**--vcpus**=*VCPUS*

    Number of vCPUs for installation VM

**--karg**=*KARG*

    Additional kernel arguments for installation

**-c**, **--connect**=*CONNECT*

    Hypervisor connection URI (e.g., qemu:///system, qemu+ssh://host/system)

<!-- END GENERATED OPTIONS -->

# EXAMPLES

TODO: Add practical examples showing how to use this command.

# SEE ALSO

**bcvk**(8)

# VERSION

v0.1.0

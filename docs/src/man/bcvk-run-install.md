# NAME

bcvk-run-install - Install bootc images to persistent disk images

# SYNOPSIS

**bcvk run-install** \[**-h**\|**\--help**\] \[*OPTIONS*\] *IMAGE*

# DESCRIPTION

Performs automated installation of bootc containers to disk images
using ephemeral VMs as the installation environment. Supports multiple
filesystems, custom sizing, and creates bootable disk images ready
for production deployment.

The installation process:

1. Creates a new disk image with the specified filesystem layout
2. Boots an ephemeral VM with the target container image
3. Runs \`bootc install to-disk\` within the VM to install to the disk
4. Produces a bootable disk image that can be deployed anywhere

# OPTIONS

<!-- BEGIN GENERATED OPTIONS -->
**SOURCE_IMAGE**

    Container image to install

    This argument is required.

**TARGET_DISK**

    Target disk/device path

    This argument is required.

**--filesystem**=*FILESYSTEM*

    Root filesystem type (e.g. ext4, xfs, btrfs)

**--root-size**=*ROOT_SIZE*

    Root filesystem size (e.g., '10G', '5120M')

**--storage-path**=*STORAGE_PATH*

    Path to host container storage (auto-detected if not specified)

**--disk-size**=*DISK_SIZE*

    Disk size to create (optional, defaults to calculated size based on source image)

**--memory**=*MEMORY*

    Memory size (e.g. 2G, 1024M, 512m, or plain number for MB)

    Default: 2048

**--vcpus**=*VCPUS*

    Number of vCPUs

    Default: 32

**--karg**=*KERNEL_ARGS*

    Additional kernel command line arguments

**--net**=*NET*

    Network configuration (none, user, bridge=name) [default: none]

**--console**=*CONSOLE*

    Enable console output to terminal for debugging

    Possible values:
    - true
    - false

**--debug**=*DEBUG*

    Enable debug mode (drop to shell instead of running QEMU)

    Possible values:
    - true
    - false

**--virtio-serial-out**=*NAME:FILE*

    Add virtio-serial device with output to file (format: name:/path/to/file)

**--execute**=*EXECUTE*

    Execute command inside VM via systemd and capture output

**-K**, **--ssh-keygen**=*SSH_KEYGEN*

    Generate SSH keypair and inject via systemd credentials

    Possible values:
    - true
    - false

**--label**=*LABEL*

    Add metadata to the container in key=value form

<!-- END GENERATED OPTIONS -->

# ARGUMENTS

*IMAGE*

:   Container image reference to install (e.g., \`registry.example.com/my-bootc:latest\`)

# EXAMPLES

Install a bootc image to a disk image:

    bcvk run-install quay.io/example/my-bootc:latest

Install with custom output path and filesystem:

    bcvk run-install --output /path/to/disk.img --filesystem btrfs registry.example.com/bootc:prod

# VERSION

v0.1.0
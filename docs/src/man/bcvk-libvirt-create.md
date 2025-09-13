# NAME

bcvk-libvirt-create - Create and start domains from uploaded bootc volumes

# SYNOPSIS

**bcvk libvirt create** [*OPTIONS*]

# DESCRIPTION

Create and start domains from uploaded bootc volumes

# OPTIONS

<!-- BEGIN GENERATED OPTIONS -->
**VOLUME_NAME_OR_IMAGE**

    Name of the bootc volume to use for domain creation, OR container image to create from If this looks like a container image (contains '/' or ':'), will automatically upload if needed

    This argument is required.

**--pool**=*POOL*

    Libvirt storage pool name

    Default: default

**--domain-name**=*DOMAIN_NAME*

    Name for the libvirt domain (defaults to volume name)

**--memory**=*MEMORY*

    Memory size for the domain (e.g. 2G, 1024M)

    Default: 2G

**--vcpus**=*VCPUS*

    Number of vCPUs for the domain

    Default: 32

**--network**=*NETWORK*

    Network configuration (default, bridge=name, none)

    Default: default

**--start**=*START*

    Start the domain after creation

    Possible values:
    - true
    - false

**--vnc**=*VNC*

    Enable VNC console access

    Possible values:
    - true
    - false

**-c**, **--connect**=*CONNECT*

    Hypervisor connection URI (e.g., qemu:///system, qemu+ssh://host/system)

**--vnc-port**=*VNC_PORT*

    VNC port (default: auto-assign)

**--karg**=*KARG*

    Additional kernel arguments

**--dry-run**=*DRY_RUN*

    Dry run - validate configuration without creating domain

    Possible values:
    - true
    - false

**--force**=*FORCE*

    Force creation even if domain already exists

    Possible values:
    - true
    - false

**--filesystem**=*FILESYSTEM*

    Root filesystem type (e.g. ext4, xfs, btrfs)

**--root-size**=*ROOT_SIZE*

    Root filesystem size (e.g., '10G', '5120M')

**--storage-path**=*STORAGE_PATH*

    Path to host container storage (auto-detected if not specified)

**--disk-size**=*DISK_SIZE*

    Size of the disk image for automatic upload (e.g., '20G', '10240M')

**--install-memory**=*INSTALL_MEMORY*

    Memory size for installation VM during auto-upload (e.g. 2G, 1024M)

    Default: 2048

**--install-vcpus**=*INSTALL_VCPUS*

    Number of vCPUs for installation VM during auto-upload

    Default: 32

**--generate-ssh-key**=*GENERATE_SSH_KEY*

    Generate ephemeral SSH keypair and inject into domain

    Possible values:
    - true
    - false

**--ssh-key**=*SSH_KEY*

    Path to existing SSH private key to use (public key must exist at <key>.pub)

**--ssh-port**=*SSH_PORT*

    SSH port for port forwarding (default: auto-assign)

<!-- END GENERATED OPTIONS -->

# EXAMPLES

TODO: Add practical examples showing how to use this command.

# SEE ALSO

**bcvk**(8)

# VERSION

v0.1.0

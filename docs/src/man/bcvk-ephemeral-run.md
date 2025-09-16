# NAME

bcvk-ephemeral-run - Run bootc containers as ephemeral VMs

# SYNOPSIS

**bcvk ephemeral run** [*OPTIONS*]

# DESCRIPTION

Run bootc containers as ephemeral VMs

# OPTIONS

<!-- BEGIN GENERATED OPTIONS -->
**IMAGE**

    Container image to run as ephemeral VM

    This argument is required.

**--memory**=*MEMORY*

    Memory size (e.g. 4G, 2048M, or plain number for MB)

    Default: 4G

**--vcpus**=*VCPUS*

    Number of vCPUs

**--karg**=*KERNEL_ARGS*

    Additional kernel command line arguments

**--net**=*NET*

    Network configuration (none, user, bridge=name) [default: none]

**--console**

    Enable console output to terminal for debugging

**--debug**

    Enable debug mode (drop to shell instead of running QEMU)

**--virtio-serial-out**=*NAME:FILE*

    Add virtio-serial device with output to file (format: name:/path/to/file)

**--execute**=*EXECUTE*

    Execute command inside VM via systemd and capture output

**-K**, **--ssh-keygen**

    Generate SSH keypair and inject via systemd credentials

**-t**, **--tty**

    Allocate a pseudo-TTY for container

**-i**, **--interactive**

    Keep STDIN open for container

**-d**, **--detach**

    Run container in background

**--rm**

    Automatically remove container when it exits

**--name**=*NAME*

    Assign a name to the container

**--label**=*LABEL*

    Add metadata to the container in key=value form

**--bind**=*HOST_PATH[:NAME]*

    Bind mount host directory (RW) at /run/virtiofs-mnt-<name>

**--ro-bind**=*HOST_PATH[:NAME]*

    Bind mount host directory (RO) at /run/virtiofs-mnt-<name>

**--systemd-units**=*SYSTEMD_UNITS_DIR*

    Directory with systemd units to inject (expects system/ subdirectory)

**--log-cmdline**

    Log full podman command before execution

**--bind-storage-ro**

    Mount host container storage (RO) at /run/virtiofs-mnt-hoststorage

**--add-swap**=*ADD_SWAP*

    Allocate a swap device of the provided size

**--mount-disk-file**=*FILE[:NAME]*

    Mount disk file as virtio-blk device at /dev/disk/by-id/virtio-<name>

<!-- END GENERATED OPTIONS -->

# EXAMPLES

TODO: Add practical examples showing how to use this command.

# SEE ALSO

**bcvk**(8)

# VERSION

<!-- VERSION PLACEHOLDER -->

# NAME

bcvk-run-ephemeral-ssh - Run ephemeral VM and immediately SSH into it with lifecycle binding

# SYNOPSIS

**bcvk run-ephemeral-ssh** [*OPTIONS*]

# DESCRIPTION

Run ephemeral VM and immediately SSH into it with lifecycle binding

# OPTIONS

<!-- BEGIN GENERATED OPTIONS -->
**IMAGE**

    Container image to run as ephemeral VM

    This argument is required.

**SSH_ARGS**

    SSH command to execute (optional, defaults to interactive shell)

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

**-t**, **--tty**=*TTY*

    Allocate a pseudo-TTY for container

    Possible values:
    - true
    - false

**-i**, **--interactive**=*INTERACTIVE*

    Keep STDIN open for container

    Possible values:
    - true
    - false

**-d**, **--detach**=*DETACH*

    Run container in background

    Possible values:
    - true
    - false

**--rm**=*RM*

    Automatically remove container when it exits

    Possible values:
    - true
    - false

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

**--log-cmdline**=*LOG_CMDLINE*

    Log full podman command before execution

    Possible values:
    - true
    - false

**--bind-storage-ro**=*BIND_STORAGE_RO*

    Mount host container storage (RO) at /run/virtiofs-mnt-hoststorage

    Possible values:
    - true
    - false

**--mount-disk-file**=*FILE[:NAME]*

    Mount disk file as virtio-blk device at /dev/disk/by-id/virtio-<name>

<!-- END GENERATED OPTIONS -->

# EXAMPLES

TODO: Add practical examples showing how to use this command.

# SEE ALSO

**bcvk**(8)

# VERSION

v0.1.0

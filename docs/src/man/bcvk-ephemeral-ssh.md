# NAME

bcvk-ephemeral-ssh - Connect to running VMs via SSH

# SYNOPSIS

**bcvk ephemeral ssh** [*OPTIONS*]

# DESCRIPTION

Connect to running ephemeral VMs via SSH. This command provides SSH access to VMs created by **bcvk-ephemeral-run**(8).

## VM Lifecycle Management

When using SSH with ephemeral VMs, the VM lifecycle can be bound to the SSH connection depending on how the VM was started:

- **Background VMs** (started with `-d`): The VM continues running independently after SSH disconnection
- **Interactive VMs** (started without `-d`): The VM terminates when SSH disconnects
- **Auto-cleanup VMs** (started with `--rm`): The VM and container are automatically removed when the VM stops

For the **bcvk-ephemeral-run-ssh**(8) command, the VM lifecycle is tightly coupled to the SSH session - when the SSH client terminates (e.g., by running `exit`), the entire VM and its container are automatically cleaned up.

# OPTIONS

<!-- BEGIN GENERATED OPTIONS -->
**CONTAINER_NAME**

    Name or ID of the container running the target VM

    This argument is required.

**ARGS**

    SSH arguments like -v, -L, -o

<!-- END GENERATED OPTIONS -->

# EXAMPLES

Connect to a running ephemeral VM:

    bcvk ephemeral ssh mytestvm

Connect with SSH verbose mode:

    bcvk ephemeral ssh mytestvm -v

Connect with port forwarding:

    bcvk ephemeral ssh mytestvm -L 8080:localhost:80

VM lifecycle examples:

    # Start a background VM (continues after SSH disconnect)
    bcvk ephemeral run -d --name persistent-vm quay.io/fedora/fedora-bootc:42
    bcvk ephemeral ssh persistent-vm
    # VM keeps running after 'exit'
    
    # Start an auto-cleanup VM (removes when stopped)
    bcvk ephemeral run -d --rm --name temp-vm quay.io/fedora/fedora-bootc:42
    bcvk ephemeral ssh temp-vm
    # VM and container auto-removed when VM stops
    
    # For tightly-coupled lifecycle, use run-ssh instead
    bcvk ephemeral run-ssh quay.io/fedora/fedora-bootc:42
    # VM terminates automatically when SSH session ends

# SEE ALSO

**bcvk**(8), **bcvk-ephemeral**(8), **bcvk-ephemeral-run**(8), **bcvk-ephemeral-run-ssh**(8)

# VERSION

v0.1.0
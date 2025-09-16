# NAME

bcvk-libvirt-ssh - SSH to libvirt domain with embedded SSH key

# SYNOPSIS

**bcvk libvirt ssh** [*OPTIONS*]

# DESCRIPTION

SSH to libvirt domain with embedded SSH key

# OPTIONS

<!-- BEGIN GENERATED OPTIONS -->
**DOMAIN_NAME**

    Name of the libvirt domain to connect to

    This argument is required.

**COMMAND**

    Command to execute on remote host

**-c**, **--connect**=*CONNECT*

    Hypervisor connection URI (e.g., qemu:///system, qemu+ssh://host/system)

**--user**=*USER*

    SSH username to use for connection (defaults to 'root')

    Default: root

**--strict-host-keys**

    Use strict host key checking

**--timeout**=*TIMEOUT*

    SSH connection timeout in seconds

    Default: 30

<!-- END GENERATED OPTIONS -->

# EXAMPLES

TODO: Add practical examples showing how to use this command.

# SEE ALSO

**bcvk**(8)

# VERSION

v0.1.0

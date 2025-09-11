# NAME

bcvk-libvirt-list - List available bootc volumes with metadata

# SYNOPSIS

**bcvk libvirt list** [*OPTIONS*]

# DESCRIPTION

List available bootc volumes with metadata

# OPTIONS

<!-- BEGIN GENERATED OPTIONS -->
**--pool**=*POOL*

    Libvirt storage pool name to search

    Default: default

**--json**=*JSON*

    Output format (human-readable or JSON)

    Possible values:
    - true
    - false

**--detailed**=*DETAILED*

    Show detailed volume information

    Possible values:
    - true
    - false

**--source-image**=*SOURCE_IMAGE*

    Filter by source container image

**--all**=*ALL*

    Show all volumes (not just bootc volumes)

    Possible values:
    - true
    - false

**-c**, **--connect**=*CONNECT*

    Hypervisor connection URI (e.g., qemu:///system, qemu+ssh://host/system)

<!-- END GENERATED OPTIONS -->

# EXAMPLES

TODO: Add practical examples showing how to use this command.

# SEE ALSO

**bcvk**(8)

# VERSION

v0.1.0

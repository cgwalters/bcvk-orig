# NAME

bcvk-pb-run - Run a bootable container as a persistent VM

# SYNOPSIS

**bcvk pb run** [*OPTIONS*]

# DESCRIPTION

Run a bootable container as a persistent VM

# OPTIONS

<!-- BEGIN GENERATED OPTIONS -->
**IMAGE**

    Container image to run as a bootable VM

    This argument is required.

**--name**=*NAME*

    Name for the VM (auto-generated if not specified)

**--memory**=*MEMORY*

    Memory size in MB for the VM

    Default: 2048

**--cpus**=*CPUS*

    Number of virtual CPUs for the VM

    Default: 2

**--disk-size**=*DISK_SIZE*

    Disk size for the VM (e.g. 20G, 10240M, or plain number for bytes)

    Default: 20G

**--filesystem**=*FILESYSTEM*

    Root filesystem type for installation

    Default: ext4

**-p**, **--port**=*PORT_MAPPINGS*

    Port mapping from host to VM

**-v**, **--volume**=*VOLUMES*

    Volume mount from host to VM

**--network**=*NETWORK*

    Network mode for the VM

    Default: user

**--detach**

    Keep the VM running in background after creation

**--ssh**

    Automatically SSH into the VM after creation

<!-- END GENERATED OPTIONS -->

# EXAMPLES

TODO: Add practical examples showing how to use this command.

# SEE ALSO

**bcvk**(8)

# VERSION

v0.1.0

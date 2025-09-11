# NAME

bcvk - A comprehensive toolkit for developing and testing bootc containers

# SYNOPSIS

**bcvk** \[**-h**\|**\--help**\] \<*subcommands*\>

# DESCRIPTION

A comprehensive toolkit for developing and testing bootc containers.

bcvk provides a complete workflow for building, testing, and managing
bootc containers using ephemeral VMs. Run bootc images as temporary VMs,
install them to disk, or manage existing installations - all without
requiring root privileges.

The toolkit includes commands for:

- Running ephemeral VMs for testing container images
- Installing bootc containers to persistent disk images
- Managing libvirt integration and VM lifecycle
- Executing host commands from within containers
- SSH access to running VMs

# OPTIONS

<!-- BEGIN GENERATED OPTIONS -->
<!-- END GENERATED OPTIONS -->

# SUBCOMMANDS

bcvk-hostexec(8)

:   Execute commands on the host system from within containers

bcvk-images(8)

:   Manage and inspect bootc container images

bcvk-run-ephemeral(8)

:   Run bootc containers as temporary VMs for testing and development

bcvk-run-install(8)

:   Install bootc images to persistent disk images

bcvk-libvirt(8)

:   Manage libvirt integration for bootc containers

bcvk-ssh(8)

:   Connect to running VMs via SSH

bcvk-help(8)

:   Print this message or the help of the given subcommand(s)

# VERSION

v0.1.0
# A toolkit for developing bootc containers

## Initial work

As of right now this only implements part of
<https://github.com/containers/podman-bootc/issues/99>

## Running a bootc container as ephemeral VM 

This doesn't require any privileges, it's just a wrapper
for `podman`. It does require a virt stack (qemu, virtiofsd)
in the host environment.

```
./target/release/bootc-kit run-ephemeral --rm -ti quay.io/fedora/fedora-bootc:42 --karg=systemd.unit=rescue.target --karg=systemd.setenv=SYSTEMD_SULOGIN_FORCE=1
```

## Goals

This project aims to implement
<https://gitlab.com/fedora/bootc/tracker/-/issues/2>.

Related projects and content:

- https://github.com/coreos/coreos-assembler/
- https://github.com/ublue-os/bluefin-lts/blob/main/Justfile

## Development

See docs/HACKING.md


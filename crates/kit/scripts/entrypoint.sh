#!/bin/bash
# 
set -euo pipefail

# Shell script library

SELFEXE=/run/selfexe

# Create a stub rootfs, mounting the host /usr
init_tmproot() {
    if test -d /run/tmproot; then return 0; fi
    mkdir /run/tmproot
    cd /run/tmproot

    # Bind mount host /usr to our hybrid root
    mkdir usr
    mount --bind /run/hostusr usr
    # Create essential symlinks that typically point to /usr
    ln -sf usr/bin bin
    ln -sf usr/lib lib
    ln -sf usr/lib64 lib64
    ln -sf usr/sbin sbin
    mkdir -p {etc,var,dev,proc,run,sys,tmp}

    # This directory is shared between the outer container (podman) and the inner (bwrap)
    mkdir /run/inner-shared
}

BWRAP_ARGS=(
    --bind /run/tmproot /
    --proc /proc
    --dev-bind /dev /dev
    --tmpfs /run
    --tmpfs /tmp
    --bind /run/inner-shared /run/inner-shared
    )

runbwrap() {
    bwrap "${BWRAP_ARGS[@]}" "$@"
}

init_tmproot

# Propagate all of /run
# Pass CLI arguments from outer container
runbwrap --bind /run /run -- ${SELFEXE} run-ephemeral-impl \
    --memory "${BOOTC_MEMORY}" \
    --vcpus "${BOOTC_VCPUS}" \
    ${BOOTC_EXTRA_ARGS:+--extra-args "${BOOTC_EXTRA_ARGS}"} \
    ${BOOTC_CONSOLE:+--console}

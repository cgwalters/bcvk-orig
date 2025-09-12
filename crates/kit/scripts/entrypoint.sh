#!/bin/bash
set -euo pipefail

SELFEXE=/run/selfexe

# Shell script library
init_tmproot() {
    if test -d /run/tmproot; then return 0; fi
    mkdir /run/tmproot
    cd /run/tmproot

    # Bind mount host /usr to our hybrid root
    mkdir usr
    mount --bind /run/hostusr usr
    # Create essential symlinks
    ln -sf usr/bin bin
    ln -sf usr/lib lib
    ln -sf usr/lib64 lib64
    ln -sf usr/sbin sbin
    mkdir -p {etc,var,dev,proc,run,sys,tmp}
    # Ensure we have /etc/passwd as ssh-keygen wants it for bad reasons
    systemd-sysusers --root $(pwd) &>/dev/null

    # Shared directory between containers
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

# Initialize environment
init_tmproot

# Pass ALL arguments to container-entrypoint
# Default to "run-ephemeral" if no args (backward compatibility)
if [[ $# -eq 0 ]]; then
    set -- "run-ephemeral"
fi

# Execute with proper environment passing
exec bwrap --as-pid-1 "${BWRAP_ARGS[@]}" --bind /run /run -- ${SELFEXE} container-entrypoint "$@"

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
    --bind /var/tmp /var/tmp
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

# Gather information about the target environment
export SYSTEMD_VERSION=$(systemctl --version)

# Execute with proper environment passing
# Set up signal handlers that will cleanly exit on INT or TERM
trap 'kill -TERM $BWRAP_PID 2>/dev/null; exit 0' INT TERM

# Run bwrap in background so we can handle signals; xref
# https://github.com/containers/bubblewrap/pull/586
# But probably really we should switch to systemd
bwrap --as-pid-1 --unshare-pid "${BWRAP_ARGS[@]}" --bind /run /run -- ${SELFEXE} container-entrypoint "$@" &
BWRAP_PID=$!

# Wait for bwrap to complete
wait $BWRAP_PID
EXIT_CODE=$?

# Exit with the same code as bwrap
exit $EXIT_CODE

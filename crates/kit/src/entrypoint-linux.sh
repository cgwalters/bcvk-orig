#!/bin/bash
set -euo pipefail
# Set of args for podman. We always kill the container on
# exit, and pass stdin.
args=(--rm -i)
# If stdin is a terminal, then tell podman to make one too.
if [ -t 0 ]; then
    args+=(-t)
fi


# Allow overriding the image.
BOOTC_KIT_IMAGE=${BOOTC_KIT_IMAGE:-ghcr.io/bootc-dev/kit}
# Isolation/security options. In the general case we need to spawn
# things on the host.
args+=(--net=host --privileged --pid=host)
# Mounts we bind to get access to host functionality
args+=(-v ${XDG_RUNTIME_DIR}/bus:/run/bus --env=DBUS_SESSION_BUS_ADDRESS=unix:path=/run/bus)
# However by default keep the image read only, just on general principle.
# We need to pass through the host /tmp as we use it to communicate with libvirt
args+=(--read-only --read-only-tmpfs -v /tmp:/tmp)
# Default to passing through the current working directory.
args+=(-v $(pwd):/run/context -w /run/context)
# And spawn the container.
exec podman run ${BCK_CONTAINER_ARGS:-} ${args[@]} "${BOOTC_KIT_IMAGE}" "$@"

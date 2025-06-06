#!/usr/bin/env python3

# This script is injected by cloud-init and reprovisions the system
# using system-reinstall-bootc. Most of it is generating systemd units.

import os
import subprocess
from pathlib import Path

BASE_IMAGE_NAME = os.environ["BOOTC_TARGET_IMAGE"]
CSTOR_DIST_PORT = os.environ.get("BOOTC_CSTOR_DIST_PORT", "")

# To help debugging
autologin_conf = """\
[Service]
ExecStart=
ExecStart=-/usr/sbin/agetty --autologin root --noclear %I $TERM
"""
# 1. Configure autologin for the serial console
autologin_dir = Path("/etc/systemd/system/serial-getty@ttyS0.service.d")
autologin_dir.mkdir(parents=True, exist_ok=True)
(autologin_dir / "autologin.conf").write_text(autologin_conf)

tls_verify_flag = ""
if CSTOR_DIST_PORT != "":
    tls_verify_flag = "--tls-verify=false"
pull_image = """\
#!/bin/bash

for attempt in $(seq 120); do
  hostip=$(ip route | grep -Ee '^default' | awk '{ print $3 }' || true)
  if test -n "$hostip"; then
    image=${hostip}:CSTOR_DIST_PORT/BASE_IMAGE_NAME
    echo BOOTC_IMAGE=$image > /run/bootc-container-target
    break
  fi
done

# Ensure podman is available; install via dnf if not found.
dnf -y install podman
# Retry loop for pulling the image (up to 60 attempts).
for attempt in $(seq 120); do
  # Check if image already exists (using 'inspect' as 'exists' is deprecated).
  if podman image inspect $image >/dev/null 2>&1; then exit 0; fi;
  sleep 1;
  # Attempt to pull the image; if successful, exit the loop.
  podman pull TLS_VERIFY_FLAG $image && exit 0
done
# If the loop completes without successfully pulling the image, exit with an error.
exit 1
""".replace("BASE_IMAGE_NAME", BASE_IMAGE_NAME).replace("CSTOR_DIST_PORT", CSTOR_DIST_PORT).replace("TLS_VERIFY_FLAG", tls_verify_flag)
Path("/usr/local/bin/bootc-pull-image").write_text(pull_image)
os.chmod("/usr/local/bin/bootc-pull-image", 0o755)

bootc_reinstall_pull_content = """\
[Unit]
Description=Pull bootc container image with retries
After=network-online.target
Wants=network-online.target

[Service]
Type=oneshot
ExecStart=/usr/local/bin/bootc-pull-image

[Install]
WantedBy=multi-user.target
"""
Path("/etc/systemd/system/bootc-reinstall-pull.service").write_text(bootc_reinstall_pull_content)

def run_command(cmd_args, **kwargs):
    """Helper to run a subprocess, print its execution, and check for errors."""
    print(f"Executing: {' '.join(cmd_args)}")
    subprocess.run(cmd_args, check=True, **kwargs)

bootc_reinstall_content = f"""\
[Unit]
Description=Install bootc image to root and reboot
After=bootc-reinstall-pull.service
Requires=bootc-reinstall-pull.service

[Service]
Type=oneshot
EnvironmentFile=/run/bootc-container-target
ExecStart=podman run --rm --privileged -v /dev:/dev -v /:/target -v /var/lib/containers:/var/lib/containers --pid=host --security-opt label=type:unconfined_t $BOOTC_IMAGE bootc install to-existing-root --skip-fetch-check
ExecStart=/usr/sbin/reboot

[Install]
WantedBy=multi-user.target
"""
Path("/etc/systemd/system/bootc-reinstall.service").write_text(bootc_reinstall_content)

run_command(["systemctl", "daemon-reload"])
run_command(["systemctl", "enable", "--now", "--no-block", "bootc-reinstall.service"])

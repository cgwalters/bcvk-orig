project := "bootc-dev/kit"
image := "localhost/" + project

# Creates a container image build
build *ARGS:
    podman build -t {{ image }} {{ ARGS }} .

unittest *ARGS:
    podman build --jobs=4 --target units -t {{ image }}-units --build-arg=unitargs={{ARGS}} .

# Run this before committing
fmt:
    cargo fmt

# This is a wrapper for bck because it overrides the default image
run *ARGS: build
    env BOOTC_KIT_IMAGE={{ image }} bck {{ ARGS }}


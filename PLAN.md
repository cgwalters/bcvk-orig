Based on your query and the provided sources, we can refine the bootc-sdk project plan to prioritize a native host binary with an externally provided virtualization (virt) stack by default, while also offering a bundled container image for specific scenarios. This approach addresses the tension between ergonomic local development and robust CI/CD and disconnected environments
.
--------------------------------------------------------------------------------
Revised Plan for Project
I. Project Vision and Scope (Updated)
The bootc-sdk aims to provide a comprehensive toolkit for developing, provisioning, and testing immutable image-based Linux systems using bootable containers.
- Primary Deployment Model: The main interface will be a native host binary for an ergonomic user experience on developer machines
.
- Virt Stack Management (Default): This native binary will expect the virtualization stack (e.g., QEMU, libvirt) to be present and configured on the host system
. The podman-bootc binary already links to libvirt.so, making it less portable anyway, which supports this approach
.
- Alternative Deployment (Containerized): The tool will also be designed to be easily containerized, bundling the SDK binary itself and the virtualization stack within a single container image. This is crucial for seamless execution in container-native environments like CI/CD pipelines (e.g., GitHub Actions, Tekton/Konflux) and Kubernetes, where /dev/kvm can be mounted
. This also simplifies disconnected environments by allowing users to mirror one image
.
II. Core Functionality: Local QEMU on Linux (Focus on Host-Provided Virt)
The bootc-sdk will leverage and build upon the podman-bootc pull request #95 for robust local VM capabilities
.
1. VM Provisioning Mechanism:
    - Host-Driven VM Launch: The bootc-sdk (native host binary) will directly orchestrate the launch of VMs using host-provided libvirt and QEMU.
    - Containerized bootc Execution: The bootc install command itself will still run inside a container that mounts the target bootc image's filesystem
. This leverages podman's capabilities.
    - Addressing Rootless Podman Challenges: The issue of libvirt on the host not being able to access container filesystems mounted with unshare in rootless podman is a key challenge
. This plan addresses it by making the bootc image available to the VM through mechanisms like image volumes
.
2. Booting the VM with Target Image as RootFS:
    - Direct Kernel Boot from Target: The VM will be booted directly using the Linux kernel and initrd extracted from the target bootc container image itself
. This minimizes "skew" and ensures compatibility, as using a separate "builder VM" image with a different kernel can lead to issues with filesystem types or features. This is considered a significant advantage, avoiding the need to maintain a separate builder VM image
.
    - Dynamic Injection of Dependencies: Instead of building an intermediate image layer with dependencies for the installation VM (which could cause GC and concurrency issues), the bootc-sdk will dynamically inject necessary tooling and configuration into a derived container image on top of the target bootc image
. This minimizes global state side effects and simplifies caching. For example, socat can be replaced by a tiny, statically compiled binary or by direct VSOCK support within podman-remote if it becomes available
.
    - Image Volumes for RootFS: The bootc-sdk will utilize podman's image volume functionality (--mount=type=image) to expose the bootc image's filesystem directly to the container where bootc install runs
. This avoids the need to extract large image files every time
.
    - Output Disk Image: The output disk image (e.g., QCOW2) will be passed into the VM and identified by the device /dev/virtio-output
. Users will need to pre-create the output disk file (e.g., qemu-img create -f qcow2 <target> <size>) before running the installation command. The bootc-sdk can then run bootc install to-disk
.
3. Communication with the VM:
    - VSOCK Proxy: The initial approach used a VSOCK proxy to bridge unix sockets on the host to VSOCK ports inside the VM, enabling remote control of the podman instance running within the VM
.
    - Addressing Security & Portability Concerns: Concerns about VSOCK's global availability and security implications were raised
.
        * virtio-serial is a preferred alternative as it offers more scoped access, limiting exposure to the specific process with the file descriptor
.
        * Ongoing kernel efforts to introduce namespaces for VSOCK CIDs could mitigate some global availability risks
.
        * The proxy implementation can be refined (e.g., using tcpproxy
or a custom, statically compiled binary), or ideally, podman-remote could gain direct VSOCK support. For now, the focus will be on Linux
.
4. Executing bootc install and User Experience:
    - The bootc-sdk will remotely control the podman instance inside the VM to execute the bootc install command
.
    - Simplified Command Line Interface: The CLI will be designed to be concise and user-friendly. The config-dir requirement should be removed, as the target configuration is expected to come from the image itself
.
    - Rootless Podman Support: The bootc-sdk will transparently handle rootless podman environments on the host, automatically spinning up podman machine if necessary for disk image building or other privileged operations
.
III. Supporting Multiple Installation Methods
While local QEMU with to-disk is the initial focus, the bootc-sdk will be designed to integrate with and automate other bootc installation methods
.
1. bootc install to-disk (Current Focus): This method, which generates a disk image directly, will be fully supported
. The bootc-sdk will manage the orchestration for various disk image formats (e.g., QCOW2, ISO, raw)
.
2. Future Integrations: The project design will leave conceptual space for other installation methods
:
    - Anaconda: Streamlined workflow for installing bootc images via the Anaconda installer
.
    - bootc-image-builder (BIB): Automate the use of bootc-image-builder for more complex disk image creation
. Projects like image-template already leverage BIB
.
    - Cloud Image Reinstallation: Offer support for reinstalling cloud images
.
IV. Addressing Cross-Environment Support & Best Practices
1. Versatile Deployment (Host Binary & Container Image):
    - The bootc-sdk will function as a native host binary for local development and direct system interaction
.
    - It will also be packaged as a container image (embedding the bootc-sdk binary itself and potentially common virt tools like libvirt and QEMU) to enable seamless execution in CI/CD pipelines (e.g., GitHub Actions
, Tekton/Konflux) and Kubernetes environments. This approach helps manage dependencies and ensures consistency
.
    - Resource sharing (podman/libvirt sockets, /dev/kvm, /dev/vsock) between the SDK container and any internal virtualization containers it manages (if that model is chosen for specific virt stack components) will be carefully managed, ideally via appropriate mounts and device allocations (--device /dev/kvm is preferred over -v /dev/kvm
)
.
2. Robustness and Security:
    - Temporary Resource Management: All temporary files and directories will be programmatically managed, created in ephemeral locations (e.g., using mktemp -d
), and properly cleaned up to prevent resource leaks
.
    - Error Handling: Implement comprehensive error checking and propagation for all operations, especially for external command executions and API interactions, to ensure failures are not silently ignored
.
    - TLS Verification: Adhere to secure practices by enabling TLS verification for image pulls by default
. Any exceptions must be explicitly justified and documented
.
    - sudo Minimization: Reduce explicit sudo calls within internal scripts by designing them to be invoked with necessary privileges from the outset by the higher-level bootc-sdk CLI
.
V. Development and Integration
1. Leveraging Existing Work: Development will heavily build upon the podman-bootc pull request #95
and the bootc-dev/bootc project's integration test efforts. The Go bindings for libvirt and podman are a strong starting point
.
2. Comprehensive Testing:
    - tmt Integration: Utilize the Test Management Tool (tmt) and its bootc provision plugin for extensive integration testing
. The tmt plugin's ability to build bootc disk images and provision VMs provides a robust testing framework
.
    - CI/CD: Implement integration tests in GitHub Actions workflows. This involves building the bootc-sdk binary, generating disk images (e.g., using bootc install to-disk
), booting VMs with QEMU, and executing tmt tests against them
.
3. Community and Documentation:
    - Maintain clear and up-to-date documentation for users and contributors
.
    - Foster collaboration with the bootc, podman, and tmt communities
.
NotebookLM can be inaccurate; please double check its responses. 

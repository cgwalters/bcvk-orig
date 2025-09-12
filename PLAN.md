# bootc-kit (bcvk) Project Plan

## Project Status Update (September 2025)

The project has been renamed from **bootc-sdk** to **bootc-kit** with the binary name `bcvk` (bootc virtualization kit). This reflects the current focus on virtualization tooling for bootc containers.

### Recent Achievements

1. **Project Restructuring**: The codebase has been reorganized into a Rust workspace with separate crates:
   - `kit`: Main CLI tool (`bcvk`)  
   - `integration-tests`: Test suite
   - `xtask`: Build automation

2. **Core Implementation**: The project now provides working commands for:
   - `bcvk run-ephemeral`: Launch ephemeral VMs from bootc containers
   - `bcvk to-disk`: Create persistent bootable disk images  
   - `bcvk ssh`: Connect to running VMs
   - `bcvk images list`: List bootc container images

3. **Documentation**: Added comprehensive man pages and documentation infrastructure imported from bootc.

4. **Integration with podman-bootc**: The project complements [podman-bootc](https://github.com/bootc-dev/podman-bootc) by providing:
   - Direct host integration (no podman-machine requirement on Linux)
   - Streamlined CLI for common virtualization tasks
   - Focus on container-to-VM workflows

## Project Vision and Scope (Updated)

bootc-kit provides a comprehensive toolkit for developing, testing, and deploying immutable image-based Linux systems using bootable containers.

### Key Differentiators vs podman-bootc

| Feature | bootc-kit (bcvk) | podman-bootc |
|---------|------------------|--------------|
| Platform | Linux host-native | Cross-platform (macOS, Linux, Windows) |
| Dependencies | Host QEMU/libvirt | Podman Machine + rootful mode |
| Use Case | Direct Linux development | Cross-platform development |
| Architecture | Single native binary | Go binary with libvirt bindings |

## Current Implementation Status

### Core Functionality: Local QEMU on Linux

The current implementation provides:

1. **VM Provisioning Mechanism**:
   - **Host-Driven VM Launch**: Direct orchestration of VMs using host-provided QEMU and virtiofsd
   - **Container Integration**: Leverages podman for container operations while using host virtualization
   - **Ephemeral VMs**: Quick container-to-VM launch without persistent storage
   - **Persistent Disk Creation**: `bootc install to-disk` workflow for creating bootable images
2. **Technical Implementation Details**:
   - **Direct Kernel Boot**: VMs boot using kernel and initrd extracted from the target bootc container
   - **VSOCK Communication**: Uses VSOCK for host-VM communication (implemented in current codebase)
   - **Virtiofs Integration**: Leverages virtiofsd for filesystem sharing between host and VM
   - **SSH Access**: Automatic SSH key injection and connection management

3. **User Experience**:
   - **Simple CLI**: Commands like `bcvk run-ephemeral` and `bcvk ssh` provide intuitive workflows
   - **No Privilege Escalation**: Runs without requiring root privileges on the host
   - **Fast Iteration**: Ephemeral VMs enable rapid container-to-VM testing cycles
## Roadmap and Future Development

### Immediate Goals (Q4 2024 - Q1 2025)
- ✅ **Core CLI Implementation**: Basic `run-ephemeral`, `to-disk`, and `ssh` commands
- ✅ **Documentation**: Man pages and user guides  
- ⏳ **Testing Infrastructure**: Integration tests using tmt framework
- ⏳ **CI/CD Pipeline**: Automated testing and releases

### Medium-term Goals (2025)
1. **Enhanced Installation Methods**:
   - Support for additional disk formats beyond QCOW2
   - Integration with bootc-image-builder (BIB) for complex image creation
   - Cloud image workflows

2. **Improved User Experience**:
   - Better error handling and diagnostics
   - Performance optimizations for large container images
   - Configuration management and profiles

3. **Ecosystem Integration**:
   - tmt provision plugin compatibility
   - GitHub Actions integration examples
   - Container registry optimization

### Next: Async Process Management Refactoring (Q1 2025)

**Priority: High** - Refactor the "inner" process supervision system to use async/await patterns for improved performance and maintainability.

#### Current State Analysis
The process supervision system currently uses synchronous patterns with manual threading:
- **Sequential Process Spawning**: QEMU and virtiofsd are launched synchronously
- **Blocking Operations**: Socket waiting and process monitoring block execution threads
- **Manual Cleanup**: Resource cleanup uses polling-based approaches

#### Refactoring Goals
1. **Concurrent Process Management**: Launch QEMU and virtiofsd concurrently using `tokio::process`
2. **Structured Concurrency**: Use `tokio::select!` and `JoinSet` for coordinated process lifecycle management  
3. **Async Resource Cleanup**: Replace polling-based cleanup with async event-driven patterns
4. **Improved Error Handling**: Leverage async error propagation for better failure management

#### Implementation Plan
- **Phase 1**: Convert core process spawning in `qemu.rs` to async patterns
- **Phase 2**: Refactor VM orchestration in `run_ephemeral.rs` to use async coordination
- **Phase 3**: Update process supervision and cleanup with structured concurrency
- **Phase 4**: Add comprehensive async testing patterns

#### Expected Benefits  
- **Performance**: 60-80% reduction in VM startup time through concurrent process initialization
- **Resource Efficiency**: Better CPU utilization and reduced thread overhead
- **Maintainability**: Cleaner error handling and shutdown procedures
- **Scalability**: Foundation for handling multiple concurrent VM operations

### Long-term Vision
- **Cross-platform Support**: While currently Linux-focused, explore containerized deployment for broader platform support
- **Advanced Virtualization**: Support for nested virtualization, GPU passthrough, and specialized hardware
- **Declarative Configuration**: YAML/TOML configuration files for complex deployment scenarios

## Development Principles

### Security and Robustness
- **Temporary Resource Management**: Proper cleanup of temporary files and directories
- **Error Handling**: Comprehensive error checking for all operations
- **TLS Verification**: Secure image pulls by default
- **Minimal Privileges**: No unnecessary privilege escalation

### Testing and Quality Assurance
- **Integration Testing**: Using tmt framework for comprehensive VM testing
- **CI/CD**: Automated testing in GitHub Actions
- **Documentation**: Maintain up-to-date user and developer documentation

### Community and Collaboration
- Active collaboration with bootc, podman, and tmt communities
- Clear contribution guidelines and development documentation
- Regular community feedback incorporation

---

**Note**: This plan reflects the current state as of September 2025. The project has successfully transitioned from concept to working implementation, providing a solid foundation for the roadmap ahead. 

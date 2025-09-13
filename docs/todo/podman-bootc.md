# Podman-bootc Drop-in Replacement Implementation

## Overview
Implement a `bcvk pb` (podman-bootc) subcommand that provides full drop-in replacement functionality for the podman-bootc tool, leveraging our existing libvirt and QEMU infrastructure.

## Core Commands to Implement

### `bcvk pb run <image>`
Primary command to create and run a bootable container VM.

**Target Interface:**
```bash
# Basic usage
bcvk pb run quay.io/centos-bootc/centos-bootc:stream9
bcvk pb run quay.io/fedora/fedora-bootc:42

# With options
bcvk pb run --filesystem xfs <image>
bcvk pb run --name my-vm <image>
bcvk pb run --memory 4096 --cpus 2 <image>
bcvk pb run --port 8080:80 <image>
bcvk pb run --volume /host/path:/container/path <image>
```

**Implementation Plan:**
1. **CLI Interface**: Add `PodmanBootc` subcommand with `Run` nested command
2. **VM Lifecycle**: Create persistent VM (not ephemeral like current `run-ephemeral`)
3. **Disk Image Creation**: Convert container to bootable disk image using our existing flow
4. **VM Management**: Start VM and maintain state (running/stopped)
5. **SSH Integration**: Auto-inject SSH keys and provide connection details

### `bcvk pb ssh <vm-name>`
SSH into a running podman-bootc VM.

**Target Interface:**
```bash
# SSH to named VM
bcvk pb ssh my-vm

# SSH to default/latest VM
bcvk pb ssh

# SSH with command execution
bcvk pb ssh my-vm -- systemctl status
```

**Implementation Plan:**
1. **VM Discovery**: Find running VMs by name or default to latest
2. **SSH Connection**: Use existing SSH infrastructure from `ssh.rs`
3. **Port Discovery**: Query libvirt for SSH port forwarding details
4. **Command Execution**: Support command execution like existing SSH command

### Additional Commands

```bash
bcvk pb list           # List all podman-bootc VMs
bcvk pb stop <name>    # Stop a running VM  
bcvk pb start <name>   # Start a stopped VM
bcvk pb remove <name>  # Remove VM and its disk image
bcvk pb inspect <name> # Show VM details and status
```

**CRITICAL FIX - `bcvk pb list` Implementation:**

**Problem:** Current implementation uses VmRegistry as source of truth, which can become inconsistent with actual libvirt domain state.

**Solution:** Implemented libvirt-first approach:
1. **New module**: `podman_bootc/domain_list.rs` - Queries libvirt directly using `virsh list --all`
2. **Domain filtering**: Identifies podman-bootc domains by bootc metadata in domain XML
3. **Real-time state**: Always shows current libvirt domain state (running/stopped/etc.)
4. **Metadata extraction**: Extracts container image, memory, vcpu info from domain XML
5. **No cache dependency**: Works even if VmRegistry cache is missing or stale

**Implementation Status:**
- ✅ `domain_list.rs` module created with libvirt domain querying
- ✅ Updated `list_vms()` function to use `DomainLister`
- ✅ Fixed domain creation to use `DomainBuilder` with proper bootc metadata
- ✅ Detection now correctly uses XML metadata only (no heuristics)
- ✅ New domains created by `bcvk pb run` will have proper bootc metadata in XML

## Architecture and Implementation Strategy

### 1. CLI Structure Enhancement

**New CLI modules:**
```rust
// In main.rs - add to Commands enum
PodmanBootc(PodmanBootcCommand),

#[derive(Parser)]
pub struct PodmanBootcCommand {
    #[command(subcommand)]
    command: PodmanBootcSubCommand,
}

#[derive(Subcommand)]
pub enum PodmanBootcSubCommand {
    Run(PodmanBootcRunCommand),
    Ssh(PodmanBootcSshCommand),
    List,
    Stop { name: String },
    Start { name: String },
    Remove { name: String },
    Inspect { name: String },
}
```

### 2. VM State Management

**ISSUE IDENTIFIED**: The VmRegistry approach creates a second source of truth that can get out of sync with libvirt domains.

**SOLUTION**: Use libvirt as the single source of truth for domain listing and state management.

**Updated approach:**
- **Primary source**: Query libvirt directly for all domain information (`virsh list --all`)
- **Domain filtering**: Use libvirt domain metadata to identify podman-bootc domains
- **State synchronization**: Always query libvirt for current domain state
- **Registry as cache**: Use VmRegistry only for supplementary metadata (creation details, source image), never as primary source

**VM Metadata Structure:**
```rust
#[derive(Serialize, Deserialize)]
pub struct PodmanBootcVm {
    name: String,
    image: String,
    created: SystemTime,
    libvirt_domain: Option<String>,
    ssh_port: Option<u16>,
    memory_mb: u32,
    vcpus: u32,
    disk_path: PathBuf,
    status: VmStatus,
}

#[derive(Serialize, Deserialize)]
pub enum VmStatus {
    Created,
    Running,
    Stopped,
}
```

### 3. Reusable Components from Existing Codebase

**Direct reuse:**
- **QEMU management**: `qemu.rs` - `QemuConfig` and `RunningQemu`
- **SSH infrastructure**: `ssh.rs` and `sshcred.rs` - key generation and injection
- **Libvirt integration**: `libvirt/` - domain creation and management
- **VirtioFS**: For container-to-VM filesystem mounting
- **Container execution**: Modified version of `run_ephemeral.rs` flow

**Adaptation needed:**
- **Persistent storage**: Create actual disk images instead of ephemeral VirtioFS
- **VM lifecycle**: Long-running VMs vs. ephemeral execution
- **Networking**: Enhanced port forwarding and network configuration

### 4. Image to VM Conversion Flow

**Enhanced bootc install process:**
1. **Container preparation**: Pull and prepare bootable container
2. **Disk image creation**: Create qcow2 disk image (default 20GB, configurable)
3. **bootc install**: Use container's `bootc install` to write to disk image
4. **VM creation**: Create libvirt domain with the disk image
5. **SSH setup**: Inject SSH keys via systemd credentials during first boot
6. **Network setup**: Configure port forwarding for SSH access

### 5. SSH Integration Enhancements

**Current SSH capabilities to leverage:**
- **Key generation**: `generate_ssh_keypair()` from `ssh.rs`
- **Systemd credentials**: SMBIOS injection via `smbios_cred_for_root_ssh()`
- **Connection testing**: Existing SSH validation logic

**Enhancements needed:**
- **VM-specific SSH configs**: Store SSH details per VM in metadata
- **Automatic connection**: Auto-discover SSH ports for named VMs
- **Key management**: Per-VM SSH keys or shared keys

### 6. Storage and Networking

**Storage strategy:**
- **Disk images**: Store in `~/.cache/bootc-kit/podman-bootc/disks/`
- **qcow2 format**: Default format with optional raw/vmdk support
- **Size configuration**: Default 20GB, configurable via `--disk-size`
- **Volume mounting**: Support for `--volume` host-to-guest bind mounts

**Networking strategy:**
- **Default**: User-mode networking with SSH port forwarding (like current)
- **Port mapping**: Support `--port host:guest` syntax
- **Bridge support**: Optional bridge networking for advanced use cases
- **Network isolation**: Support `--network none` for isolated VMs

## Implementation Tasks Breakdown

### Phase 1: Core Infrastructure
1. **CLI structure**: Add `PodmanBootc` command and subcommands
2. **VM registry**: Implement persistent VM metadata storage
3. **Disk image creation**: Adapt existing container-to-disk flow
4. **Basic run command**: Implement `bcvk pb run <image>` with libvirt

### Phase 2: SSH and Management
1. **SSH command**: Implement `bcvk pb ssh <name>`
2. **VM discovery**: Name-based VM lookup and management
3. **VM lifecycle**: Start/stop/remove commands
4. **Status tracking**: VM state management and persistence

### Phase 3: Advanced Features
1. **Volume mounting**: Support `--volume` bind mounts
2. **Port forwarding**: Implement `--port` syntax
3. **Resource limits**: Memory/CPU configuration
4. **List and inspect**: VM enumeration and detailed status

### Phase 4: Testing and Validation
1. **Unit tests**: Test individual components
2. **Integration tests**: End-to-end podman-bootc compatibility
3. **Multi-distro testing**: Test with various bootc images
4. **Performance validation**: Compare with original podman-bootc

## Testing Strategy

### Unit Tests
- **VM metadata serialization/deserialization**
- **CLI argument parsing**
- **SSH key generation and injection**
- **Libvirt domain XML generation**

### Integration Tests
- **Full VM lifecycle**: Create, start, SSH, stop, remove
- **Multi-VM management**: Multiple VMs with different names
- **Container compatibility**: Test with official bootc images:
  - `quay.io/centos-bootc/centos-bootc:stream9`
  - `quay.io/fedora/fedora-bootc:42`
- **SSH functionality**: Connection, command execution, exit codes
- **Error handling**: Invalid images, name conflicts, resource limits

### Compatibility Tests
- **CLI compatibility**: Ensure `bcvk pb` matches `podman-bootc` behavior
- **Image compatibility**: Support same container images
- **Feature parity**: Match key functionality and options

## Success Criteria

### Functional Requirements
1. **Drop-in replacement**: `bcvk pb run <image>` works identically to `podman-bootc run <image>`
2. **SSH access**: `bcvk pb ssh <name>` provides seamless shell access
3. **VM persistence**: VMs remain running after command completion
4. **State management**: Track and manage multiple named VMs
5. **Container compatibility**: Works with standard bootc container images

### Performance Requirements
1. **VM creation time**: Comparable to original podman-bootc (within 20%)
2. **SSH connection time**: Sub-5 second connection establishment
3. **Resource efficiency**: No significant memory/CPU overhead vs. original
4. **Disk space**: Efficient qcow2 storage with minimal overhead

### Quality Requirements
1. **Error handling**: Clear error messages for common failure modes
2. **Documentation**: Complete CLI help and usage examples
3. **Test coverage**: >80% coverage of core functionality
4. **Compatibility**: Pass existing podman-bootc test suites where applicable

## Technical Risks and Mitigations

### Risk: Container Image Compatibility
- **Issue**: Not all container images are bootc-compatible
- **Mitigation**: Validate bootc metadata, provide clear error messages

### Risk: VM State Corruption
- **Issue**: libvirt domain state could become inconsistent
- **Mitigation**: Atomic state updates, recovery mechanisms

### Risk: SSH Key Management
- **Issue**: SSH key injection might fail on some systemd versions
- **Mitigation**: Multiple injection methods, fallback strategies

### Risk: Resource Conflicts
- **Issue**: Multiple VMs competing for ports/resources
- **Mitigation**: Dynamic port allocation, resource validation

## Current Implementation Plan (Refined)

Based on research of bootc-kit and podman-bootc SSH injection mechanisms, here's the refined implementation approach:

### Immediate Tasks (Current Sprint)

#### 1. SSH Key Management System Design
- **Ephemeral SSH Keys**: Generate unique SSH keypairs per VM (stored in libvirt domain XML annotations)
- **Key Storage**: Store private key path in domain metadata, public key injected via SMBIOS
- **Configurable Keys**: Support `--ssh-key` option to use existing keypair
- **Default Behavior**: Auto-generate ephemeral keys if none specified

#### 2. Enhanced libvirt create with SSH Integration
- **Add SMBIOS support**: Integrate existing `sshcred.rs` SMBIOS credential injection
- **Domain XML enhancement**: Add QEMU commandline args for SMBIOS type=11
- **SSH key options**: Add `--ssh-key` and `--generate-ssh-key` flags to libvirt create
- **Annotation storage**: Store SSH key metadata in libvirt domain annotations

#### 3. New `bcvk libvirt ssh` Command
- **SSH connection**: Connect to libvirt domains using stored SSH metadata
- **Port discovery**: Query domain XML for SSH port forwarding configuration
- **Key lookup**: Retrieve SSH private key path from domain annotations
- **Command execution**: Support remote command execution like existing SSH command

#### 4. Enhanced `bcvk pb run` Integration
- **Leverage libvirt create**: Use `bcvk libvirt create --start --generate-ssh-key`
- **Automatic SSH setup**: Auto-inject SSH keys and configure port forwarding
- **Seamless connection**: After VM creation, immediately SSH to the VM
- **Persistent VMs**: Create libvirt domains that persist beyond command execution

### Technical Implementation Details

#### SSH Key Injection via SMBIOS (Existing Mechanism)
Using bootc-kit's existing `sshcred.rs` implementation:
```rust
// Generate SMBIOS credential string for SSH key injection
let smbios_cred = smbios_cred_for_root_ssh(&ssh_pubkey)?;

// Add to QEMU command line via libvirt
let qemu_args = format!(
    r#"<qemu:commandline>
    <qemu:arg value='-smbios'/>
    <qemu:arg value='type=11,value={}'/>
</qemu:commandline>"#,
    smbios_cred
);
```

#### Domain XML Annotation Storage
Store SSH metadata in libvirt domain XML:
```xml
<metadata>
  <bootc:container xmlns:bootc="https://github.com/containers/bootc">
    <bootc:ssh-private-key>/path/to/private/key</bootc:ssh-private-key>
    <bootc:ssh-port>2222</bootc:ssh-port>
    <bootc:generated-key>true</bootc:generated-key>
  </bootc:container>
</metadata>
```

#### QEMU Command Integration
Extend domain builder to support QEMU commandline arguments:
```rust
// In domain.rs - add method to DomainBuilder
pub fn with_qemu_args(mut self, args: &[String]) -> Self {
    self.qemu_args = Some(args.to_vec());
    self
}

// Generate XML with qemu:commandline namespace
```

### Implementation Sequence

#### Step 1: Enhance libvirt create (In Progress)
- Add SSH key generation options to `LibvirtCreateOpts`
- Integrate SMBIOS credential injection from `sshcred.rs`
- Add QEMU commandline support to `DomainBuilder`
- Store SSH metadata in domain XML annotations

#### Step 2: Create libvirt ssh command
- New CLI command: `bcvk libvirt ssh <domain-name>`
- Read SSH metadata from domain XML annotations
- Establish SSH connection using stored private key
- Support command execution: `bcvk libvirt ssh <domain> -- <command>`

#### Step 3: Update pb run integration
- Modify `bcvk pb run` to use `bcvk libvirt create --start --generate-ssh-key`
- Auto-SSH after successful VM creation
- Maintain VM registry for podman-bootc compatibility

#### Step 4: Testing and Validation
- Test SSH key injection mechanism
- Verify systemd credential processing in VMs
- Test end-to-end: run → create domain → SSH connection
- Validate with multiple bootc images

### Success Metrics
1. **SSH Key Injection**: Auto-generated keys work seamlessly with systemd credentials
2. **libvirt ssh Command**: Can SSH to any domain created with SSH keys
3. **pb run Integration**: Single command creates VM and provides SSH access
4. **Persistent VMs**: Domains remain available after command completion

### Next Immediate Actions
1. Add SSH key generation options to `LibvirtCreateOpts`
2. Integrate SMBIOS credential injection into domain creation
3. Implement QEMU commandline support in `DomainBuilder`
4. Create basic `bcvk libvirt ssh` command
5. Test end-to-end SSH injection and connection
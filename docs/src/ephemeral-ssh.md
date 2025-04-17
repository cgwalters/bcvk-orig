# SSH Workflow Concepts

SSH access to ephemeral VMs enables powerful development and debugging workflows. This guide covers the concepts, patterns, and strategies for effectively using SSH with bcvk ephemeral VMs.

## SSH in the Ephemeral VM Context

### What Makes VM SSH Different

SSH to ephemeral VMs differs from traditional SSH in several ways:
- **Dynamic targets**: VMs are created and destroyed frequently
- **Automatic key management**: Keys are generated and injected automatically
- **Network isolation**: VMs may have limited network access
- **Lifecycle coupling**: SSH sessions can control VM lifetime

### SSH Workflow Models

**Direct SSH**: Create VM and immediately connect via SSH
**Named VM SSH**: Connect to existing named VMs
**Session-coupled**: VM lifecycle tied to SSH session
**Persistent access**: VM continues after SSH disconnection

## Core SSH Concepts

### Automatic Key Management

bcvk handles SSH key complexity:
- **Key generation**: Automatically creates key pairs when needed
- **Key injection**: Injects public keys into VM during creation
- **Secure access**: Uses ephemeral keys for security
- **No manual setup**: No need to manually configure SSH access

### Connection Strategies

**Immediate connection**: Start VM and SSH in one operation
**Deferred connection**: Create VM first, connect later
**Multiple sessions**: Connect multiple times to same VM
**Forwarded connections**: Use SSH tunneling for services

### Network Context

SSH connections work within network constraints:
- **Isolated VMs**: SSH works even without external network access
- **Port forwarding**: Access VM services through SSH tunnels
- **Host integration**: Bridge between host and VM environments
- **Security boundaries**: Maintain isolation while enabling access

## Common SSH Workflows

### Quick Debugging Workflow

The fastest way to debug a bootc image:

```bash
# Create VM, SSH directly, cleanup on exit
bcvk ephemeral ssh quay.io/myapp/debug:latest
```

**Use cases**:
- Quick image testing
- Boot debugging
- Configuration verification
- One-time troubleshooting

**Benefits**:
- Immediate access
- Automatic cleanup
- No persistent state
- Fast iteration

### Development Environment Workflow

For persistent development work:

```bash
# Create development VM
bcvk ephemeral run -d --name dev-env \
  --bind ~/code:/workspace \
  quay.io/myapp/dev:latest

# Connect when needed
bcvk ephemeral ssh dev-env
```

**Use cases**:
- Ongoing development
- Multiple SSH sessions
- File sharing between host and VM
- Long-running development tasks

**Benefits**:
- Persistent environment
- Multiple concurrent connections
- Host directory integration
- Background operation

### Testing and Validation Workflow

For automated testing scenarios:

```bash
# Create test VM with specific configuration
bcvk ephemeral run -d --name test-vm \
  --memory 2048 --port 8080:80 \
  quay.io/myapp/test:latest

# Run tests via SSH
bcvk ephemeral ssh test-vm "cd /app && ./run-tests.sh"

# Additional validation
bcvk ephemeral ssh test-vm "systemctl status myapp"
```

**Use cases**:
- Automated testing
- Service validation
- Configuration testing
- Integration verification

**Benefits**:
- Scriptable access
- Remote command execution
- Service interaction
- Automated validation

## SSH Key Management Concepts

### Automatic Key Generation

When you use SSH with ephemeral VMs:
1. **Key creation**: New key pair generated automatically
2. **Public key injection**: Public key added to VM's authorized_keys
3. **Private key storage**: Private key stored securely on host
4. **Automatic cleanup**: Keys removed when VM is destroyed

### Key Security Model

**Ephemeral keys**: Keys exist only for VM lifetime
**Isolated access**: Each VM gets unique keys
**No key reuse**: Fresh keys for each VM creation
**Automatic rotation**: New keys for new VMs

### Manual Key Management

When you need specific keys:
- **Existing keys**: Use your own public key files
- **Shared access**: Multiple users with different keys
- **Key rotation**: Update keys on running VMs
- **Access control**: Manage who can connect

## SSH Integration Patterns

### File Transfer Integration

SSH enables seamless file movement:

**SCP for single files**:
- Quick file uploads/downloads
- Configuration file updates
- Log file retrieval
- Binary deployment

**SFTP for interactive transfer**:
- Browse VM filesystem
- Interactive file management
- Directory synchronization
- Batch operations

**rsync for directory synchronization**:
- Efficient directory mirroring
- Incremental updates
- Bandwidth optimization
- Backup operations

### Port Forwarding Integration

SSH tunneling connects host and VM services:

**Local forwarding**: Access VM services from host
- Web applications
- Database connections
- API endpoints
- Development servers

**Remote forwarding**: Access host services from VM
- Development tools
- Local databases
- Host services
- Build systems

**Dynamic forwarding**: SOCKS proxy through VM
- Network debugging
- Security testing
- Protocol analysis
- Network simulation

### Development Tool Integration

SSH integrates with development environments:

**IDE integration**:
- Remote development
- Code editing in VM
- Debugging support
- Terminal integration

**Version control**:
- Git operations in VM
- Repository synchronization
- Code deployment
- Branch testing

**Build systems**:
- Remote compilation
- Cross-platform builds
- Environment isolation
- Dependency management

## Security Considerations

### Isolation Benefits

SSH to ephemeral VMs provides security advantages:
- **Process isolation**: VM processes can't affect host
- **Network isolation**: Limited VM network access
- **Filesystem isolation**: VM filesystem separated from host
- **Resource isolation**: Controlled resource consumption

### Security Best Practices

**Key management**:
- Use ephemeral keys when possible
- Rotate keys regularly
- Limit key scope and lifetime
- Monitor key usage

**Access control**:
- Use specific user accounts in VMs
- Implement least privilege access
- Monitor SSH sessions
- Log access attempts

**Network security**:
- Use SSH tunneling for service access
- Avoid unnecessary port forwarding
- Monitor network traffic
- Implement network policies

## Troubleshooting SSH Workflows

### Common Connection Issues

**Connection refused**:
- VM not fully booted yet
- SSH service not running
- Network configuration problems
- Firewall blocking connections

**Authentication failures**:
- Key not properly injected
- Wrong user account
- SSH service configuration
- Key format problems

**Performance issues**:
- Network latency
- VM resource constraints
- SSH configuration tuning
- Concurrent connection limits

### Debugging Strategies

**Incremental debugging**:
1. Verify VM is running
2. Check SSH service status
3. Test basic connectivity
4. Validate key configuration
5. Debug specific issues

**Logging and monitoring**:
- Enable verbose SSH output
- Check VM system logs
- Monitor network connectivity
- Track resource usage

**Alternative access methods**:
- Use VM console access
- Container debugging tools
- Network troubleshooting
- Resource monitoring

## Performance Optimization

### Connection Speed

**Key factors affecting SSH performance**:
- Network latency between host and VM
- VM resource allocation
- SSH configuration settings
- Concurrent session limits

**Optimization strategies**:
- Use connection multiplexing
- Optimize SSH configuration
- Allocate adequate VM resources
- Monitor network performance

### Session Management

**Multiple sessions**:
- Use SSH connection sharing
- Implement session persistence
- Manage concurrent connections
- Balance resource usage

**Long-running sessions**:
- Configure session timeouts
- Implement session recovery
- Monitor session health
- Plan for disconnections

## Automation and Scripting

### Script-Friendly SSH

SSH workflows can be automated:

**Non-interactive execution**:
- Run commands without terminal
- Capture command output
- Handle error conditions
- Script complex workflows

**Batch operations**:
- Execute multiple commands
- Transfer multiple files
- Configure multiple VMs
- Implement deployment scripts

### CI/CD Integration

SSH enables continuous integration workflows:
- **Automated testing**: Run tests in clean VMs
- **Deployment validation**: Test deployments automatically
- **Environment provisioning**: Create test environments on demand
- **Quality assurance**: Automated quality checks

## Best Practices

### Workflow Design

- **Plan for automation**: Design workflows that can be scripted
- **Handle failures**: Plan for SSH connection failures
- **Resource management**: Clean up VMs and connections
- **Security first**: Use minimal necessary access

### Development Efficiency

- **Use named VMs**: For persistent development environments
- **Share directories**: Mount host code directories in VMs
- **Automate common tasks**: Script frequent operations
- **Monitor resources**: Track VM resource usage

### Operational Excellence

- **Document workflows**: Maintain clear documentation
- **Version control**: Keep scripts in version control
- **Test procedures**: Validate SSH workflows regularly
- **Monitor usage**: Track SSH session patterns

## Next Steps

For detailed command syntax and options, see:
- [bcvk-ephemeral-ssh(8)](./man/bcvk-ephemeral-ssh.md) - Complete command reference
- [Ephemeral VM Concepts](./ephemeral-run.md) - Understanding ephemeral VMs
- [Network Configuration](./network-config.md) - Advanced networking concepts
- [Storage Management](./storage-management.md) - Data persistence strategies
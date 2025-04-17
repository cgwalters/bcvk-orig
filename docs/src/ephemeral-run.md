# Ephemeral VM Concepts

Ephemeral VMs provide a powerful way to quickly test, develop, and experiment with bootc containers in isolated virtual machine environments. This guide covers the concepts, workflows, and use cases for running ephemeral VMs with bcvk.

## What are Ephemeral VMs?

Ephemeral VMs are temporary virtual machines that:
- Start quickly from bootc container images
- Run independently from your host system
- Automatically clean up when no longer needed
- Provide full operating system environments for testing

Think of ephemeral VMs as "containers that boot" - they give you the isolation and quick startup of containers with the full capabilities of a virtual machine.

## Core Concepts

### Container-to-VM Transformation

bcvk transforms your bootc container images into bootable VMs:

1. **Container Image**: Your application packaged as a bootc container
2. **VM Creation**: bcvk creates a temporary VM from the container
3. **Boot Process**: The container becomes a full operating system
4. **Isolation**: Complete separation from the host system

### Lifecycle Models

Ephemeral VMs support different lifecycle patterns:

**Foreground Mode**: VM runs in your terminal, exits when you stop it
**Background Mode**: VM runs independently, continues after terminal closes  
**Auto-cleanup**: VM automatically removes itself when stopped
**Persistent**: VM remains available for future use

## When to Use Ephemeral VMs

### Development and Testing

Ephemeral VMs excel for:
- **Quick testing** of bootc images before deployment
- **Development environments** that mirror production
- **Integration testing** with network isolation
- **Debugging** boot processes and system configurations

### Experimentation

Perfect for:
- **Trying new software** without affecting your host
- **Testing configurations** safely
- **Learning** new technologies in isolated environments
- **Prototyping** deployment scenarios

### CI/CD Workflows

Ideal for:
- **Automated testing** in clean environments
- **Build verification** of bootc images
- **Quality assurance** checks
- **Deployment validation**

## Common Workflow Patterns

### Quick Test Workflow

```bash
# Start VM, test, automatic cleanup
bcvk ephemeral run --rm quay.io/myapp/test:latest
```

This pattern is perfect for:
- Quick functionality tests
- One-time validation
- Continuous integration checks

### Development Workflow

```bash
# Persistent VM with development tools
bcvk ephemeral run -d --name dev-env \
  --memory 4096 --bind ~/code:/workspace \
  quay.io/myapp/dev:latest
```

This pattern provides:
- Persistent development environment
- Host directory access
- Background operation
- Resource customization

### Testing Workflow

```bash
# Isolated test environment
bcvk ephemeral run --rm --name test \
  --port 8080:80 --memory 2048 \
  quay.io/myapp/staging:latest
```

This pattern enables:
- Service testing with port forwarding
- Isolated network environment
- Controlled resource allocation
- Automatic cleanup

## Resource Management Concepts

### Memory Allocation

Memory in ephemeral VMs:
- **Default**: Usually 4GB, suitable for most testing
- **Scaling**: Adjust based on application requirements
- **Host Impact**: Allocated from host system memory
- **Performance**: More memory = better performance for memory-intensive apps

### CPU Assignment

CPU allocation considerations:
- **vCPU Count**: Virtual CPUs assigned to the VM
- **Host Sharing**: vCPUs share physical CPU cores
- **Performance**: More vCPUs help with parallel workloads
- **Resource Balance**: Match CPU to memory allocation

### Storage Behavior

Ephemeral VM storage:
- **Temporary**: Storage disappears when VM stops (unless persistent)
- **Copy-on-Write**: Changes don't affect the original container
- **Performance**: Stored in host temporary space
- **Size Planning**: Consider space for application data and logs

## Network Access Patterns

### Isolated Testing

Default network configuration provides:
- **No external access**: Safe for untrusted code
- **Host isolation**: VM cannot affect host network
- **Internal services**: VM can run services internally
- **Security**: Maximum isolation for testing

### Service Development

With port forwarding:
- **Selective access**: Expose only needed ports
- **Development testing**: Access services from host
- **Integration testing**: Connect multiple services
- **Load testing**: Test service under various conditions

### User Networking

For broader connectivity:
- **Internet access**: VM can reach external services
- **Package installation**: Download dependencies
- **External integration**: Connect to remote services
- **Realistic testing**: Mirror production network access

## Integration Strategies

### Host Directory Sharing

Mount host directories for:
- **Code development**: Edit on host, test in VM
- **Configuration sharing**: Share config files
- **Data persistence**: Keep important data on host
- **Build artifacts**: Share build outputs

### SSH Integration

SSH access enables:
- **Remote development**: Use VM as remote environment
- **File transfer**: Move files between host and VM
- **Service management**: Control services in VM
- **Log access**: Debug issues interactively

### Container Registry Integration

Working with registries:
- **Private images**: Use authenticated registries
- **Local images**: Test locally built containers
- **Image updates**: Pull latest versions for testing
- **Multi-architecture**: Test different CPU architectures

## Best Practices

### Resource Planning

- **Start small**: Begin with default resources, scale as needed
- **Monitor usage**: Check actual resource consumption
- **Host capacity**: Ensure sufficient host resources
- **Cleanup**: Remove unused VMs to free resources

### Security Considerations

- **Isolation first**: Use default isolated networking when possible
- **Minimal exposure**: Only forward necessary ports
- **Trusted images**: Use known, trusted container images
- **Regular cleanup**: Don't leave test VMs running indefinitely

### Development Efficiency

- **Named VMs**: Use descriptive names for persistence
- **Background mode**: Run development VMs in background
- **Host integration**: Mount relevant host directories
- **Quick iteration**: Use ephemeral VMs for rapid testing cycles

### Automation Integration

- **Scripted testing**: Automate VM creation for tests
- **CI integration**: Use in continuous integration pipelines
- **Parametrization**: Script common configurations
- **Error handling**: Plan for VM startup failures

## Troubleshooting Concepts

### Common Issues

**VM Won't Start**:
- Check image availability and validity
- Verify host resources (memory, disk space)
- Ensure container runtime is working

**Poor Performance**:
- Increase memory allocation
- Add more vCPUs
- Check host system load
- Verify storage performance

**Network Problems**:
- Understand chosen network mode
- Check port forwarding configuration
- Verify firewall settings
- Test connectivity step by step

**Storage Issues**:
- Monitor disk space usage
- Check temporary directory permissions
- Verify container image integrity
- Consider storage performance limitations

### Debugging Strategies

- **Start simple**: Begin with minimal configurations
- **Incremental changes**: Add complexity gradually
- **Log analysis**: Check both host and VM logs
- **Resource monitoring**: Track CPU, memory, and disk usage
- **Network testing**: Verify connectivity at each layer

## Comparison with Alternatives

### vs. Regular Containers

Ephemeral VMs provide:
- **Full OS environment**: Complete operating system
- **Better isolation**: Hardware-level separation
- **Boot testing**: Test actual boot processes
- **Network isolation**: More sophisticated networking

### vs. Persistent VMs

Ephemeral VMs offer:
- **Faster setup**: No pre-configuration needed
- **Automatic cleanup**: No manual management
- **Consistent state**: Always start from clean image
- **Lower overhead**: Only exist when needed

### vs. Cloud Instances

Ephemeral VMs provide:
- **Local execution**: No network dependencies
- **Faster iteration**: Immediate startup
- **Cost efficiency**: No cloud charges
- **Development focus**: Optimized for development workflows

## Next Steps

For detailed command syntax and options, see:
- [bcvk-ephemeral-run(8)](./man/bcvk-ephemeral-run.md) - Complete command reference
- [Ephemeral SSH Access](./ephemeral-ssh.md) - SSH connection workflows
- [Libvirt Integration](./libvirt-integration.md) - Persistent VM alternatives
- [Storage Management](./storage-management.md) - Advanced storage concepts
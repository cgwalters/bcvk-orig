# Workflow Comparison

The bootable container ecosystem includes several tools that serve different use cases. Understanding when to use each tool helps you choose the right approach.

## Tool Overview

- **bootc** - Core tool for building and managing bootable container images
- **bcvk** - Virtualization toolkit for development and testing
- **podman-bootc** - Podman-integrated solution for cross-platform development

## Quick Comparison

| Tool | Best For | Key Strength |
|------|----------|-------------|
| **bootc** | Production deployment | Direct hardware installation |
| **bcvk** | Development/testing | Fast VM workflows |
| **podman-bootc** | Cross-platform dev | Consistent experience |

## When to Use bcvk

- **Development iteration**: Quick testing of container changes
- **Linux-focused workflows**: Leveraging native virtualization
- **Integration testing**: Automated VM testing in CI/CD
- **Performance testing**: Native KVM performance

## When to Use Alternatives

- **podman-bootc**: Cross-platform teams, Podman workflows
- **bootc**: Production deployment, bare metal installation

Most teams use multiple tools: bcvk for development, bootc for production.
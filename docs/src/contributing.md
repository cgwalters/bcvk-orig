# Contributing

We welcome contributions to bcvk! Please see [docs/HACKING.md](../HACKING.md) for detailed development instructions.

## Quick Start

1. **Clone and build**:
   ```bash
   git clone https://github.com/cgwalters/bootc-kit.git
   cd bootc-kit
   cargo build
   ```

2. **Run tests**:
   ```bash
   # Unit tests
   just test
   
   # Integration tests (requires virtualization)
   just test-integration
   ```

3. **Make changes and test**:
   ```bash
   cargo fmt
   cargo clippy
   cargo test
   ```

## Contributing Guidelines

- Follow [conventional commits](https://www.conventionalcommits.org/) format
- Run `cargo fmt` before committing
- Include tests for new functionality
- Update documentation for user-facing changes

See [HACKING.md](../HACKING.md) for complete development guidelines and project structure details.
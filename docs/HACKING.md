# Working on this project

Be sure you've read [README.md](`../README.md`) too of course.

## Building

- There is a `Justfile` which supports commands, read it and use it.
  This wraps generic tools like `cargo check`, `cargo build`, and `cargo test` to verify
  the project compiles and unit tests work. This assumes these tools are in the current
  host environment.
- Use `just test-integration` to run the integration tests.

## Testing

### Unit tests
```bash
just test
```

### Integration tests
```bash
# Run all integration tests
just test-integration

# Run a specific integration test
just test-integration-single <test_name>

# Examples:
just test-integration-single run_ephemeral_with_storage
just test-integration-single to_disk
```

Integration tests require QEMU/KVM to be fully working as they launch actual VMs.

## Running

Ensure the entrypoint script is in `$PATH`, i.e. that `bck` works.

Then you can invoke `bck`.

## Code formatting

- Always run `cargo fmt` before making a git commit, and in
  general at the end of a series of code edits.

## Code style

Some use of emoji is OK, but avoid using it gratuitously. Especially
don't use bulleted lists where each entry has an emoji prefix.

## Commit messages

The commit message should be structured as follows:

<type>[optional scope]: <description>

[optional body]

[optional footer]

The commit contains the following structural elements, to communicate intent to the consumers of your library:

- fix: a commit of the type fix patches a bug in your codebase (this correlates with PATCH in semantic versioning).
- feat: a commit of the type feat introduces a new feature to the codebase (this correlates with MINOR in semantic versioning).
- BREAKING CHANGE: a commit that has the text BREAKING CHANGE: at the beginning of its optional body or footer section introduces a breaking API change (correlating with MAJOR in semantic versioning). A breaking change can be part of commits of any type. e.g., a fix:, feat: & chore: types would all be valid, in addition to any other type.

DO NOT include `Generated with Claude Code` or `Co-authored-by: Claude`.
You should include `Assisted-by: Claude <noreply@anthropic.com>` though
especially for nontrivial changes that did not require substantial assistance from
a human.

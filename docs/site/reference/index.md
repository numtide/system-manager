# Reference

Technical reference documentation for System Manager. For step-by-step guides, see [Tutorials](../tutorials/getting-started.md). For task-oriented instructions, see [How-to Guides](../how-to/install.md).

## Sections

### [CLI Commands](cli.md)

Complete reference for all command-line options and subcommands: `init`, `switch`, `register`, `build`, `deactivate`, `pre-populate`, `activate`, and `sudo` integration.

### [Module Options](modules.md)

Reference for all configuration options available in `.nix` modules:

- `environment.systemPackages` - System packages
- `environment.etc` - Files in `/etc`
- `systemd.services` - Systemd service definitions
- `systemd.tmpfiles` - Temporary files and directories
- `nix.settings` - Nix configuration options

### [Configuration Patterns](configuration.md)

Project organization patterns, folder structures, and workflows for managing System Manager configurations.

### [Supported Platforms](supported-platforms.md)

Platform compatibility matrix, system requirements, and distribution-specific notes.

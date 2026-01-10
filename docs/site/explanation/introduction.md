# Introduction

> Declarative system configuration for Linux distributions.

System Manager brings the power of NixOS-style declarative configuration to other Linux distributions. Define your packages, services, and `/etc` files in Nix, and System Manager ensures your system matches that configuration.

## Why System Manager?

- **No reinstall required** - Keep your existing distro (Ubuntu, Debian, Fedora, etc.) while gaining declarative configuration
- **Reproducible systems** - Your configuration files fully describe your system state
- **Generational rollback** - Switch between previous configurations when needed
- **Familiar to NixOS users** - Uses the same module system and configuration patterns

## Who is it for?

System Manager is ideal for:

- Developers who want reproducible development environments
- Sysadmins managing fleets of non-NixOS Linux servers
- NixOS users who need to configure systems where NixOS isn't an option
- Anyone tired of imperative configuration drift

## Next Steps

- [Getting Started](../tutorials/getting-started.md) - Configure your first system
- [Installation](../how-to/install.md) - Install Nix and get started
- [Examples](../examples/index.md) - See practical use cases

## Learn More

- [Declarative Configuration](declarative-config.md) - Understanding the declarative paradigm
- [How System Manager Works](how-it-works.md) - Architecture and internals
- [Comparison with NixOS](nixos-comparison.md) - When to use which

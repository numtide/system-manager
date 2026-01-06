# Introduction

**Declarative system configuration for non-NixOS Linux distributions.**

System Manager brings the power of NixOS-style declarative configuration to other Linux distributions running systemd. Define your packages, services, and /etc files in Nix, and System Manager ensures your system matches that configuration.

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

- [Getting Started](getting-started.md) - Configure your first system
- [System Requirements](requirements.md) - Check prerequisites
- [Examples](examples.md) - See practical use cases

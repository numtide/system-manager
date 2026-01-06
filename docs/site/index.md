---
template: home.html
search:
    exclude: true
---

# System Manager

**Declarative system configuration for any Linux distribution.**

System Manager brings the power of NixOS-style declarative configuration to any Linux system running systemd. Define your packages, services, and /etc files in Nix, and System Manager ensures your system matches that configuration.

## Why System Manager?

- **No reinstall required** - Keep your existing distro (Ubuntu, Debian, Fedora, etc.) while gaining declarative configuration
- **Reproducible systems** - Your configuration files fully describe your system state
- **Safe rollbacks** - Built on Nix, with generation-based rollback support
- **Familiar to NixOS users** - Uses the same module system and configuration patterns

## Who is it for?

System Manager is ideal for:

- Developers who want reproducible development environments
- Sysadmins managing fleets of non-NixOS Linux servers
- NixOS users who need to configure systems where NixOS isn't an option
- Anyone tired of imperative configuration drift

## Get Started

Ready to try it? Head to the [Getting Started](getting-started.md) guide to configure your first system in minutes.

Need to check requirements first? See the [System Requirements](install.md) page.

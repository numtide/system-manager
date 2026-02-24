# How System Manager Works

This page explains the architecture and internal workings of System Manager.

## Overview

System Manager has two main components:

1. **system-manager** - The CLI tool you interact with
2. **system-manager-engine** - The privileged core that modifies the system

When you run `nix run 'github:numtide/system-manager' -- switch --flake . --sudo`, several things happen.

## The Build Phase

First, Nix evaluates your configuration:

1. Your `flake.nix` is parsed
2. All imported modules are loaded and merged
3. The final configuration is evaluated
4. Nix builds derivations for all packages, service files, and `/etc` entries

This happens without root privileges. The output is a store path in `/nix/store/` containing everything needed to configure your system.

## The Activation Phase

With `--sudo`, System Manager runs the privileged activation:

1. **Create a new generation** - The new configuration is registered as a "generation" in `/nix/var/nix/profiles/system-manager-profiles/`

2. **Update `/etc` files** - Managed files are symlinked or copied to `/etc`. System Manager tracks which files it manages and won't touch files outside its scope.

3. **Install systemd units** - Service files are placed in `/etc/systemd/system/` and systemd is reloaded.

4. **Start/stop services** - Services that changed are restarted. New services start. Removed services stop.

5. **Update PATH** - On first activation, `/etc/profile.d/system-manager-path.sh` is created to add `/run/system-manager/sw/bin` to users' PATH.

## Generations

Every successful activation creates a new "generation." Generations are stored as Nix profiles:

```
/nix/var/nix/profiles/system-manager-profiles/
  system-manager-1-link -> /nix/store/...-system-manager-system
  system-manager-2-link -> /nix/store/...-system-manager-system
  system-manager-3-link -> /nix/store/...-system-manager-system
```

Each generation is a complete, self-contained configuration. You can switch between them instantly because all the files already exist in the Nix store.

## What System Manager Manages

System Manager can manage:

| Component | Location | How |
|-----------|----------|-----|
| Packages | `/run/system-manager/sw/bin/` | Symlinks to Nix store |
| Services | `/etc/systemd/system/` | Generated unit files |
| `/etc` files | `/etc/*` | Symlinks or copies |
| tmpfiles | `/etc/tmpfiles.d/` | tmpfiles.d configuration |

## What System Manager Doesn't Manage

System Manager intentionally limits its scope:

- **Bootloader** - Managed by your distro
- **Kernel** - Managed by your distro
- **Files outside `/etc`** - Use environment.etc for `/etc` only

This lets System Manager coexist peacefully with your distribution's package manager and configuration.

## Coexistence with Other Tools

System Manager is designed to coexist with:

- **apt/dnf/pacman** - You can use both. System Manager packages go in `/run/system-manager/sw/bin/`, distro packages go in `/usr/bin/`.
- **Manual `/etc` edits** - System Manager only touches files it's configured to manage.
- **Other Nix tools** - Works alongside home-manager, devenv, etc.

## The system-manager.target

System Manager creates a systemd target called `system-manager.target`. Services configured with `wantedBy = [ "system-manager.target" ]` start when System Manager activates and on every boot.

This target is "wanted by" `multi-user.target`, ensuring your services start during normal system boot.

## See Also

- [Introduction](introduction.md) - Why System Manager exists
- [Declarative Configuration](declarative-config.md) - The paradigm behind it
- [CLI Reference](../reference/cli.md) - Command documentation

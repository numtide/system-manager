# System Manager vs NixOS

This page explains the relationship between System Manager and NixOS, and when to use each.

## What is NixOS?

NixOS is a complete Linux distribution built on Nix. The entire system - kernel, bootloader, packages, services, users - is configured declaratively and built atomically.

## What is System Manager?

System Manager brings NixOS-style declarative configuration to *other* Linux distributions. It manages packages, services, and `/etc` files, but leaves the base system (kernel, bootloader, init) to your distribution.

If you're already running NixOS, you don't need System Manager - NixOS provides these capabilities natively.

## Key Differences

| Aspect | NixOS | System Manager |
|--------|-------|----------------|
| **Base system** | Nix all the way down | Your distro (Ubuntu, Debian, etc.) |
| **Kernel** | Managed by Nix | Managed by distro |
| **Bootloader** | Managed by Nix | Managed by distro |
| **Users/Groups** | Managed declaratively | Managed by distro |
| **Package manager** | Only Nix | Nix + distro's (apt, dnf, etc.) |
| **Init system** | systemd (required) | systemd (required) |
| **Rollback** | Full system rollback | Service/package rollback |

## When to Use NixOS

Choose NixOS when:

- You want complete declarative control over everything
- You're setting up new machines from scratch
- You want atomic system upgrades with full rollback
- You're comfortable with a less traditional Linux experience

## When to Use System Manager

Choose System Manager when:

- You can't or won't reinstall the OS
- You need to configure existing non-NixOS machines
- Your organization requires specific distributions (Ubuntu, RHEL, etc.)
- You want gradual adoption of declarative configuration
- You're managing cloud instances with vendor-supplied images

## Module Compatibility

System Manager uses a subset of NixOS modules. Many modules work directly:

```nix
# This works in both NixOS and System Manager
{
  environment.systemPackages = [ pkgs.htop ];

  systemd.services.myapp = {
    description = "My Application";
    wantedBy = [ "system-manager.target" ]; # NixOS uses different target
    serviceConfig.ExecStart = "${pkgs.myapp}/bin/myapp";
  };
}
```

Some NixOS modules don't apply to System Manager:

- `boot.*` - Bootloader configuration
- `fileSystems.*` - Filesystem mounts
- `users.users.*` - User management (though this may change)
- Hardware-specific modules

## Migration Path

### From Traditional Linux to System Manager

1. Install Nix on your existing system
2. Start managing services with System Manager
3. Gradually move more configuration to Nix
4. Keep using apt/dnf for things System Manager doesn't manage

### From System Manager to NixOS

If you later decide to switch to NixOS:

1. Your `.nix` configuration largely transfers over
2. Add NixOS-specific modules (boot, filesystems, users)
3. Install NixOS with your existing configuration as a base

### From NixOS to System Manager

If you need to configure non-NixOS machines:

1. Extract the service/package parts of your NixOS config
2. Replace NixOS-specific targets with `system-manager.target`
3. Use System Manager on your non-NixOS machines

## Using Both Together

You can use NixOS and System Manager in the same infrastructure:

- NixOS for servers you control completely
- System Manager for servers with OS requirements
- Same configuration patterns and skills for both

## See Also

- [Introduction](introduction.md) - What is System Manager?
- [How System Manager Works](how-it-works.md) - Architecture details
- [Getting Started](../tutorials/getting-started.md) - Try System Manager

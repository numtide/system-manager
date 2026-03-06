# Supported Platforms

System Manager runs on Linux systems that use systemd for service management.

## Tested Platforms

| Platform | Status | Notes |
|----------|--------|-------|
| Ubuntu 22.04+ | Tested | Primary development platform |
| Ubuntu on WSL2 | Tested | Windows Subsystem for Linux |
| NixOS | Tested | Works alongside existing NixOS configuration |
| Debian 12+ | Tested | Container-tested via systemd-nspawn |
| Fedora 42+ | Tested | Container-tested via systemd-nspawn |
| Rocky Linux 9+ | Tested | Container-tested via systemd-nspawn |
| AlmaLinux 9+ | Tested | Container-tested via systemd-nspawn |
| Arch Linux | Tested | Container-tested via systemd-nspawn (x86_64 only) |

## Requirements

### Hardware

- **Disk Space**: Minimum 12GB, recommended 16GB+
- **Memory**: Sufficient for Nix builds (2GB+ recommended)

### Software

- **Linux kernel**: Any recent version with systemd support
- **Init system**: systemd (required)
- **Nix**: System-wide multi-user installation with flakes enabled

## Platform detection

System Manager checks the platform at activation time using a pre-activation assertion that reads `/etc/os-release`.
By default, it allows activation on Ubuntu, NixOS, Debian, Fedora, Rocky Linux, AlmaLinux, and Arch Linux.

### Enabling other distributions

To allow System Manager to run on untested distributions, set the `system-manager.allowAnyDistro` option in your configuration:

```nix
{
  config = {
    system-manager.allowAnyDistro = true;
  };
}
```

This disables the OS check entirely.
There is no option to selectively allow specific distributions; the check is either on (default, allowing the tested platforms listed above) or off.

## Limitations

### Not Supported

- **Non-systemd systems**: Systems using OpenRC, runit, or other init systems
- **macOS**: System Manager is Linux-only
- **BSD**: Not supported
- **Per-user Nix installations**: System Manager requires system-wide Nix

### Known Issues

- SELinux may require additional configuration (see [Troubleshooting](../faq.md#troubleshooting))
- Some NixOS-specific modules are not available on non-NixOS systems

## See Also

- [Installation](../how-to/install.md) - How to install Nix and System Manager
- [FAQ](../faq.md) - Troubleshooting and best practices

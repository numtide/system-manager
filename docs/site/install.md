# Installation

## Requirements

To use System Manager, you need:

* **A Linux machine.** We've tested System Manager with Ubuntu both as standalone and under Windows Subsystem for Linux (WSL).
* **At least 12GB Disk Space.** However, we recommend at least 16GB, as you will be very tight for space with under 16GB. (This is primarily due to Nix; if you're using System Manager to configure, for example, small servers on the Cloud, 8GB simply won't be enough.)
* **Nix installed system-wide** with flakes enabled. (System Manager doesn't work with a per-user installation of Nix)

!!! Warning
    Rollback functionality is not yet fully implemented. While you can list and switch between generations manually, automatic rollback on failure is not available. Always test configuration changes in a VM or non-production environment first.

# Installing Nix

If you don't have Nix installed yet, use the official multi-user installer:

```sh
sh <(curl -L https://nixos.org/nix/install) --daemon
```

After installation, open a new terminal or source the profile:

```sh
. /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh
```

Verify the installation:

```sh
nix --version
```

## Enabling Flakes

The official installer does not enable flakes by default. Add this line to `/etc/nix/nix.conf`:

```ini
experimental-features = nix-command flakes
```

Alternatively, you can pass the `--extra-experimental-features` option to each `nix` command, but this is less convenient.

!!! Tip
    For other installation options (platform-specific guides, CI/CD environments), see [nix-install.com](https://nix-install.com).

## Checking Your Installation

To check if Nix is installed system-wide (required for System Manager), run:

```sh
which nix
```

If the output shows a path in your home directory (e.g., `/home/username/.nix-profile/bin/nix`), Nix is installed per-user and won't work with System Manager. A system-wide installation shows `/nix/var/nix/profiles/default/bin/nix`.

# Running System Manager

Because Nix can load code (called "flakes") remotely, you don't need to download or install System Manager. Simply running it the first time will automatically fetch it into the Nix Store (`/nix/store`).

To get started, run:

```sh
nix run 'github:numtide/system-manager' -- init
```

This will create initial configuration files in `~/.config/system-manager/`. See [Getting Started](getting-started.md) for a complete walkthrough.

## Running under sudo

System Manager needs `sudo` access to run. As such, we've provided a command-line option, `--sudo`, that allows you to grant sudo rights to System Manager.

**System Manager is still in early development, and for now the `--sudo` command line option is required.**

!!! Note
    Adding yourself to Nix's trusted-users configuration won't help here. Trusted users have elevated privileges within the Nix daemon, but System Manager requires root filesystem permissions to modify `/etc`, manage services, and install system packages. You'll still need to use sudo.

!!! Tip
    System Manager can manage your `/etc/nix/nix.conf` file for you, allowing you to declare experimental features in your `flake.nix` instead. See [Letting System Manager manage `/etc/nix/nix.conf`](reference-guide.md#letting-system-manager-manage-etcnixnixconf) for details.

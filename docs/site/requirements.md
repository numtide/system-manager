# System Requirements

In order to use System Manager, you need:

* **A Linux machine.** We've tested System Manager with Ubuntu both as standalone and under Windows Subsystem for Linux (WSL).
* **At least 12GB Disk Space.** However, we recommend at least 16GB, as you will be very tight for space with under 16GB. (This is primarily due to Nix; if you're using System Manager to configure, for example, small servers on the Cloud, 8GB simply won't be enough.)
* **Nix installed system-wide.** (System Manager doesn't work with a per-user installation of Nix)
* **Flakes** enabled

!!! Warning
    Rollback functionality is not yet fully implemented. While you can list and switch between generations manually, automatic rollback on failure is not available. Always test configuration changes in a VM or non-production environment first.

# No Installation Required

Because Nix can load code (called "flakes") remotely, you don't need to download or install System Manager. Simply running it the first time will automatically fetch it into the Nix Store (`/nix/store`), where Nix keeps all packages and their dependencies in isolation.

To get started, run:

```sh
nix run 'github:numtide/system-manager' -- init
```

This will create initial configuration files in `~/.config/system-manager/`. See [Getting Started](getting-started.md) for a complete walkthrough.

## Enabling Flakes

System Manager requires flakes to run. You can enable flakes using one of two methods:

* By adding the following line to `/etc/nix/nix.conf`:

```ini
experimental-features = nix-command flakes
```

* Or by passing the `--extra-experimental-features` option to the `nix` command:

```sh
nix run 'github:numtide/system-manager' --extra-experimental-features 'nix-command flakes' -- init
```

!!! Note
    If flakes aren't enabled in `/etc/nix/nix.conf`, the `init` subcommand will only create a `system.nix` file, not a `flake.nix` file.

!!! Tip
    System Manager can manage your `/etc/nix/nix.conf` file for you, allowing you to declare experimental features in your `flake.nix` instead. See [Letting System Manager manage `/etc/nix/nix.conf`](reference-guide.md#letting-system-manager-manage-etcnixnixconf) for details.


## Running under sudo

System Manager needs `sudo` access to run. As such, we've provided a command-line option, `--sudo`, that allows you to grant sudo rights to System Manager.

**System Manager is still in early development, and for now the `--sudo` command line option is required.**

!!! Note
    Adding yourself to Nix's trusted-users configuration won't help here. Trusted users have elevated privileges within the Nix daemon, but System Manager requires root filesystem permissions to modify `/etc`, manage services, and install system packages. You'll still need to use sudo.

## How can I tell whether Nix is installed for the whole system or just me?

Simply type:

```sh
which nix
```

If you see it's installed in your home directory, e.g.:

```console
/home/username/.nix-profile/bin/nix
```

Then it's installed just for you. Alternatively, if it's installed system-wide, you'll see:

```console
/nix/var/nix/profiles/default/bin/nix
```
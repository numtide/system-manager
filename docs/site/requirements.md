# System Requirements

In order to use System Manager, you need:

* **A Linux machine.** We've tested System Manager with Ubuntu both as standalone and under Windows Subsystem for Linux (WSL).
* **At least 12GB Disk Space.** However, we recommend at least 16GB, as you will be very tight for space with under 16GB. (This is primarily due to Nix; if you're using System Manager to configure, for example, small servers on the Cloud, 8GB simply won't be enough.)
* **Nix installed system-wide.** (System Manager doesn't work with a per-user installation of Nix)
* **Flakes** enabled

!!! Important
    System Manager does not work with the single-user installation option for Nix.

!!! Important
    At this time, System Manager requires flakes to be enabled.

!!! Warning
    Rollback functionality is not yet fully implemented. While you can list and switch between generations manually, automatic rollback on failure is not available. Always test configuration changes in a VM or non-production environment first.

# Installation

Because Nix can load code (called "flakes") remotely, you don't need to download System Manager. Simply running it the first time will automatically install it in what's called the Nix Store, which is a special directory on your system (typically `/nix/store`) where Nix keeps all packages and their dependencies in isolation.


## Regarding Experimental Features

System Manager requires flakes to run. You can enable flakes using one of two methods:

* By adding the following line to `/etc/nix/nix.conf`

```
experimental-features = nix-command flakes
```

* Or by adding the `--extra-experimental-features` option to the `nix` command line like so:

```
--extra-experimental-features 'nix-command flakes'
```

Note, however, that if you use the `init` subcommand to initialize an environment, and you do *not* have experimental features enabled in your `nix.conf` file, you will only get a default `system.nix` file, and not an associated `flake.nix` file.

!!! Recommendation
    If you need to run the `init` subcommand, but prefer to pass the `--extra-experimental-features` option to the command line, we recommend at least temporarily adding the aforementioned line to the `nix.conf` file.

!!! Important
    This is optional, but ultimately System Manager prefers to manage the `nix.conf` file for you, after which you can declare experimental features inside the `flake.nix` file as shown later in [Letting System Manager manage `/etc/nix/nix.conf`](reference-guide.md#letting-system-manager-manage-etcnixnixconf).


## Running under sudo

System Manager needs `sudo` access to run. As such, we've provided a command-line option, `--sudo`, that allows you to grant sudo rights to System Manager.

**System Manager is still in early development, and for now the `--sudo` command line option is required.**

!!! Note
    Adding yourself to Nix's trusted-users configuration won't help here. Trusted users have elevated privileges within the Nix daemon, but System Manager requires root filesystem permissions to modify `/etc`, manage services, and install system packages. You'll still need to use sudo.

## How can I tell whether Nix is installed for the whole system or just me?

Simply type

```
which nix
```


If you see it's installed off of your home directory, e.g.:

```
/home/username/.nix-profile/bin/nix
```

Then it's installed just for you. Alternatively, if it's installed for everybody, it will be installed like so:

```
/nix/var/nix/profiles/default/bin/nix
```
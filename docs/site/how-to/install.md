# Installation

## Requirements

To use System Manager, you need:

* **A Linux machine.** We've tested System Manager with Ubuntu both as standalone and under Windows Subsystem for Linux (WSL).
* **At least 12GB Disk Space.** However, we recommend at least 16GB, as you will be very tight for space with under 16GB. (This is primarily due to Nix; if you're using System Manager to configure, for example, small servers on the Cloud, 8GB simply won't be enough.)
* **Nix installed system-wide** with flakes enabled. (System Manager doesn't work with a per-user installation of Nix)

!!! Warning
    Rollback functionality is not yet fully implemented. While you can list and switch between generations manually, automatic rollback on failure is not available. Always test configuration changes in a VM or non-production environment first.

## Installing Nix

Use the official multi-user installer:

```sh
curl -sSfL https://artifacts.nixos.org/nix-installer | sh -s -- install
```

Verify the installation:

```sh
nix --version
```

### Enabling flakes

The nix-installer enables flakes by default.
If you installed Nix using a different installer, you may need to enable flakes manually by adding this line to `/etc/nix/nix.conf`:

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

This will create initial configuration files in `~/.config/system-manager/`. See [Getting Started](../tutorials/getting-started.md) for a complete walkthrough.

# Optional: Installing System Manager Locally

Nix allows you to run code that's stored remotely in a repo, such as in GitHub. As such, you don't have to install System Manager locally to use it. However, if you want to install locally, you can do so with the following `nix profile` command:

```sh
nix profile add 'github:numtide/system-manager'
```

Or, if you don't have the experimental features set in `/etc/nix/nix.conf`, you can provide them through the command line:

```sh
nix profile add 'github:numtide/system-manager' --extra-experimental-features 'nix-command flakes'
```

!!! Tip
    After System Manager is installed locally, you no longer need to worry about whether you have experimental features enabled. You will simply pass the `--flake` option to System Manager.

When you install System Manager, you might get some warnings about trusted user; this simply means you're not in the trusted user list of Nix. But System Manager will still install and work fine.

Then you can find System Manager:

```console
$ which system-manager
/home/ubuntu/.nix-profile/bin/system-manager
```

And you can run System Manager:

```sh
system-manager switch --flake . --sudo
```

!!! Tip
    System Manager is still in early development. Installing locally will not immediately pick up new changes. If you decide to install locally, periodically check the GitHub repo for changes and upgrade using `nix profile upgrade`.

# Installing on NixOS

If you're on NixOS and want to install the `system-manager` CLI as a system package, add it to your NixOS configuration:

```nix
# flake.nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    system-manager = {
      url = "github:numtide/system-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, system-manager, ... }:
    let
      system = "x86_64-linux";
    in {
      nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
        inherit system;
        modules = [
          ({ config, ... }: {
            environment.systemPackages = [
              system-manager.packages.${system}.system-manager
            ];
          })
        ];
      };
    };
}
```

Then rebuild:

```sh
sudo nixos-rebuild switch --flake .#myhost
```

# Version Compatibility

Occasionally, the nixpkgs version may be incompatible with the `main` branch of System Manager. If you encounter build errors, you may need to pin to a specific commit.

## Pinning to a Specific Version

For older nixpkgs versions (e.g., 24.05), you may need to pin System Manager to a compatible commit:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/24.05";
    system-manager = {
      type = "github";
      owner = "numtide";
      repo = "system-manager";
      ref = "64627568a52fd5f4d24ecb504cb33a51ffec086d";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
}
```

See [GitHub Issue #207](https://github.com/numtide/system-manager/issues/207) for updates on release versioning.

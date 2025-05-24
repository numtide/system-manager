<div align="center">

# system-manager

<img src="system-manager.svg" height="150"/>

**Manage system config using nix on any distro**

*A <a href="https://numtide.com/">numtide</a> project.*

<p>
<a href="https://github.com/numtide/system-manager/actions/workflows/update-flake-lock.yml"><img src="https://github.com/numtide/system-manager/actions/workflows/update-flake-lock.yml/badge.svg"/></a>
<a href="https://app.element.io/#/room/#home:numtide.com"><img src="https://img.shields.io/badge/Support-%23numtide-blue"/></a>
</p>

</div>

This project provides a basic method to manage system configuration using [Nix][nixos]
on any Linux distribution.
It builds on the many [modules that already exist in NixOS](https://github.com/NixOS/nixpkgs/tree/master/nixos/modules).

*Warning*: System Manager is a work in progress, you can expect things not to work or to break.

## Usage

### Install Nix

In order to use System Manager, you will first need to install Nix.
You can either use your distro's package manager, or use one of the available options
to install Nix.

- [Official Nix Installer][official-installer] - The canonical source for installing nix.
- [Determinate Nix Installer][detsys-installer] - A wrapper around the official installer that has SELinux support, and enables flake features by default.

> Note: Be advised that the Determinate Systems installer has the option for the official
> Nix as well as Determinate's own variant of Nix (Determinate Nix). It will prompt you
> for which one you want to install. System Manager is not tested against Determinate Nix.
> It's recommended to use the official Nix if installing via the Determinate Nix Installer.

## Usage with flakes

### Defining the configuration

A basic Nix flake using System Manager would look something like this:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    system-manager = {
      url = "github:numtide/system-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, system-manager }: {
    systemConfigs.default = system-manager.lib.makeSystemConfig {
      modules = [
        ./modules
      ];
    };
  };
}
```

And you would then put your System Manager modules in the `modules` directory,
which should contain a `default.nix` file which functions as the entrance point.

A simple System Manager module could look something like this:

```nix
{ config, lib, pkgs, ... }:

{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";

    environment = {
      etc = {
        "foo.conf".text = ''
          launch_the_rockets = true
        '';
      };
      systemPackages = [
        pkgs.ripgrep
        pkgs.fd
        pkgs.hello
      ];
    };

    systemd.services = {
      foo = {
        enable = true;
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };
        wantedBy = [ "system-manager.target" ];
        script = ''
          ${lib.getBin pkgs.hello}/bin/hello
          echo "We launched the rockets!"
        '';
      };
    };
  };
}
```

### Activating the configuration

Once the configuration is defined, you can activate it using the `system-manager` CLI:

```sh
nix run 'github:numtide/system-manager' -- switch --flake '.'
```

### Reproducibility

By design flakes run in [pure evaluation mode](https://wiki.nixos.org/wiki/Flakes#Making_your_evaluations_pure).
In some cases you may not want this. To run an impure evaluation of the flake, add the following option to your command:

```sh
--nix-option pure-eval false
```

## Currently supported features

Currently it is possible to configure files under `/etc/` and systemd services.
More features may follow later.

### Supported Systems

System Manager is currently only supported on NixOS and Ubuntu. However, it can be used on other distributions by enabling the following:

```nix
{
  config = {
    system-manager.allowAnyDistro = true;
  }
}
```

> \[!WARNING\]
> This is unsupported and untested. Use at your own risk.

## Commercial support

Looking for help or customization?

Get in touch with Numtide to get a quote. We make it easy for companies to
work with Open Source projects: <https://numtide.com/contact>

[detsys-installer]: https://github.com/DeterminateSystems/nix-installer
[nixos]: https://nixos.org
[official-installer]: https://nixos.org/download.html

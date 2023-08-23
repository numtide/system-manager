# System Manager using Nix

This project provides a basic method to manage system configuration using [Nix][nixos]
on any Linux distribution.
It builds on the many modules that already exist in [NixOS][nixos].

*Warning*: System Manager is a work in progress, you can expect things not to work or to break.

[nixos]: https://nixos.org

# Usage

## Getting Nix

In order to use System Manager, you will first need to install Nix.
You can either use your distro's package manager, or use one of the different options
to install Nix, like [the official installer][official-installer] or this
[new installer][detsys-installer].

[official-installer]: https://nixos.org/download.html
[detsys-installer]: https://github.com/DeterminateSystems/nix-installer

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

  outputs = { self, flake-utils, nixpkgs, system-manager }: {
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
    environment = {
      etc = {
        "foo.conf".text = ''
          launch_the_rockets = true
        '';
      };
      systemPackages = [
        pkgs.ripgrep
        pkgs.fd
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
          ${lib.getBin pkgs.foo}/bin/foo
          echo "We launched the rockets!"
        '';
      };
    };
  };
}
```

### Activating the configuration

Once the configuration defined, you can activate it using the `system-manager` CLI:
```sh
nix run 'github:numtide/system-manager' -- switch --flake '.'
```

# Currently supported features

Currently it is possible to configure files under `/etc/` and systemd services.
More features may follow later.

## Commercial support

Looking for help or customization?

Get in touch with Numtide to get a quote. We make it easy for companies to
work with Open Source projects: <https://numtide.com/contact>

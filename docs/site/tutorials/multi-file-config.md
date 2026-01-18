# Multi-File Configuration

This tutorial shows you how to organize your System Manager configuration across multiple files for better maintainability.

## Prerequisites

- Completed [Getting Started](getting-started.md)
- A working System Manager setup

## What You'll Learn

- How to split the configuration into separate modules
- How Nix merges configuration from multiple files
- Best practices for organizing larger configurations

## Starting Point

By default, System Manager creates two files:

```
~/.config/system-manager/
  flake.nix
  system.nix
```

This works for simple setups, but as your configuration grows, you'll want to organize it better.

## Step 1: Create a Modules Directory

Create a `modules/` folder in `~/.config/system-manager/` to hold your configuration files:

```sh
mkdir -p ~/.config/system-manager/modules
```

## Step 2: Create Separate Module Files

Let's create two separate modules, one for packages and one for services.

**modules/packages.nix**

```nix
{ pkgs, ... }:
{
  config = {
    environment.systemPackages = with pkgs; [
      bat
      tree
      htop
    ];
  };
}
```

**modules/services.nix**

```nix
{ lib, pkgs, ... }:
{
  config = {
    systemd.services.my-app = {
      description = "My Application";
      enable = true;
      wantedBy = [ "system-manager.target" ];
      serviceConfig = {
        Type = "simple";
        ExecStart = "${pkgs.hello}/bin/hello";
      };
    };
  };
}
```

## Step 3: Update Your Flake

Modify your `flake.nix` to load all modules:

```nix
{
  description = "Standalone System Manager configuration";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    system-manager = {
      url = "github:numtide/system-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, system-manager, ... }:
    {
      systemConfigs.default = system-manager.lib.makeSystemConfig {
        modules = [
          { nixpkgs.hostPlatform = "x86_64-linux"; }
          ./modules/packages.nix
          ./modules/services.nix
        ];
      };
    };
}
```

## Step 4: Apply the Configuration

```sh
nix run 'github:numtide/system-manager' -- switch --flake . --sudo
```

## How Nix Merges Configurations

When you have multiple modules, Nix automatically merges their `config` attribute sets. For example, if two files both add to `environment.systemPackages`, the packages are combined into a single list.

This means you can have:

- `modules/dev-tools.nix` with development packages,
- `modules/monitoring.nix` with monitoring tools, and
- `modules/web-server.nix` with nginx configuration.

They will all work together without conflicts.

## Recommended Structure

For larger configurations:

```
~/.config/system-manager/
  flake.nix
  modules/
    default.nix       # Common settings
    packages.nix      # System packages
    services/         # All user-defined services, one per module file.
      nginx.nix
      postgres.nix
    etc-files.nix     # /etc file management
```

## Next Steps

- Learn about [folder organization patterns](../how-to/install.md).
- See [Working with Remote Flakes](../how-to/use-remote-flakes.md) to host your config on GitHub.
- Read about [configuration options](../reference/modules.md) in the reference.

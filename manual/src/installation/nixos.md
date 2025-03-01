# NixOS Installation

## Nix Channels

This is the "vanilla" NixOS experience. You can find which channels you are currently using with `nix-channel --list`.
The configuration that NixOS uses with channels is at `/etc/nixos/configuration.nix`.

<!--
  @channels
  Remove after #207 is completed.
-->

Currently, there isn't a release plan for `system-manager` that is in tandem with nixpkgs releases. This has been an issue
in some cases that have caused failures in [_version mismatches_](https://github.com/numtide/system-manager/issues/172).

For the time being, until a release schedule is put in place that can support nix channels, please
follow the guide for [flake based configurations](#flake-based-configurations).

## Flake Based Configurations

To add `system-manager` to an existing flake based configuration, add the following to the `inputs` field of `flake.nix`.

```nix
# flake.nix
{
  inputs = {
    system-manager.url = "github:numtide/system-manager";
  };
}
```

<!--
  @channels
  Remove after #207 is completed.
-->

> NOTE: Occassionally the nixpkgs version may not be compatible with the `main` branch of `system-manager`.
> In such cases, check the current version of nixpkgs in `flake.lock` against `system-manager`.
> You may need to update the version of nixpkgs in `inputs`, or find the commit at which `system-manager` is supported
> at that version of nixpkgs and lock `system-manager` at that commit.
>
> See [Issue #207](https://github.com/numtide/system-manager/issues/207) for progress on alleviating this problem.

The `system-manager` function for creating a `system-manager` configuration is available on the `lib` field of `inputs.system-manager`.
We can create a new system configuration for a machine that includes `system-manager` like so:

<!-- TODO: Test this as some of this is based on logical assumptions -->

```nix
# flake.nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    system-manager.url = "github:numtide/system-manager";
  };
  outputs = inputs@{ self }: {
    let
      system = "aarch64-linux";
      host = "nixos";
    in
    systemManagerConfigurations.${host} = inputs.system-manager.lib.makeSystemConfig {
      modules = [
        ({ config, ... }: {
          environment.systemPackages = [
            inputs.system-manager.packages.${system}.system-manager
          ];
        })
      ];
    };
  };
}
```

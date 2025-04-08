# NixOS Installation

This section covers how to get `system-manager`, the command line application, on your system.
Please refer to [Usage](./usage.md) for how to handle application of modules.

## Nix Channels

This is the "vanilla" NixOS experience. You can find which channels you are currently using with `nix-channel --list`.
The configuration that NixOS uses with channels is at `/etc/nixos/configuration.nix`.

<!--
  @channels
  Remove after #207 is completed.
-->

Currently, there isn't a release plan for `system-manager` that is in tandem with nixpkgs releases. This has been an issue
in some cases that have caused failures in [_version mismatches_](https://github.com/numtide/system-manager/issues/172).

The only available archive is the `main` branch, which is pinned to `nixos-unstable`.
If you are currently using the unstable channel already and wish to use channels specifically you could do the following:

```sh
nix-channel --add https://github.com/numtide/system-manager/archive/main.tar.gz system-manager
nix-channel --update
nix-channel --list
# system-manager https://github.com/numtide/system-manager/archive/main.tar.gz
```

<!-- TODO: Test this, as I am just speculating that this is possible. -->

It should then be possible to add the following to `imports` in `/etc/nixos/configuration.nix` and gain access to the [`system-manager` options](https://github.com/numtide/system-manager/blob/2e5bcfaf4a8194e70bbfc9c4eda3897dc84ff3b3/nix/modules/default.nix#L17):

```nix
{ pkgs, ... }: {
  imports = [
    <system-manager/nix/modules>
    ./hardware-configuration.nix
  ];
}
```

> _**NOTICE**_: Until a release schedule is put in place that can support nix channels, it is advised to follow the guide for [flake based configurations](#flake-based-configurations) instead.

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

> NOTE: Occassionally the nixpkgs version may be incompatible with the `main` branch of `system-manager`.
> In such cases, check the current version of nixpkgs in `flake.lock` against `system-manager`.
> You may need to update the version of nixpkgs in `inputs`, or find the commit at which `system-manager` is supported
> at that version of nixpkgs and lock `system-manager` at that commit. For instance, the following commit is the only commit
> that will work for (at least) `nixpkgs-24.05` and below due to changes in Cargo's lock file parsing standard after Rust 1.83
> became available in nixpkgs:
>
> ```nix
> {
>   inputs = {
>     nixpkgs.url = "github:NixOS/nixpkgs/24.05";
>     system-manager = {
>       type = "github";
>       owner = "numtide";
>       repo = "system-manager";
>       ref = "64627568a52fd5f4d24ecb504cb33a51ffec086d";
>     };
>   };
> }
> ```
>
> See [Issue #207](https://github.com/numtide/system-manager/issues/207) for progress on alleviating this problem.

The `system-manager` package is not available via nixpkgs at present, but we can get it from the flake's `packages` attribute.
The following is a flake that declares a single NixOS configuration containing a module with the `system-manager`
package added to the environment.

<!-- TODO: Upstream system-manager into nixpkgs like home-manager -->

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
  outputs = inputs@{ self }: {
    let
      system = "aarch64-linux";
      host = "nixos";
    in
    nixosConfigurations.${host} = inputs.nixpkgs.lib.nixosSystem {
      inherit system;
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

Switching to this configuration will install `system-manager`.

```sh
sudo nixos-rebuild switch --flake ./path/to/flake.nix#nixos
system-manager --version
# system_manager 0.1.0
```

> Note: In this case our host's name is `nixos`, and to reference an attribute we tack on `#` to the flake path, followed by the name of the attribute we want to reference.

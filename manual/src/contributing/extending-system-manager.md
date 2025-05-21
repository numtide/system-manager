# Extending System Manager

## Creating an Ad-Hoc Release

There is currently not a release plan that follows nixpkgs releases, but the following ad-hoc solution is possible. The following guide is a walk-through of
how to create a release branch for some version of nixpkgs, in this case `nixpkgs-24.05`, which the commit for can be found [here](https://github.com/numtide/system-manager/compare/numtide:64ca98a...numtide:d9cd850).

1. Check that a release branch for the required version does not already exist.
1. Fork and clone the `system-manager` repository and create a new branch for the release in the following format `release-<TAG>`, for example:

```sh
git checkout -b release-24.05
```

3. Update the `nixpkgs` ref in the flake inputs with the new tag:

```diff
  # system-manager/flake.nix
  {
-   inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
+   inputs.nixpkgs.url = "github:NixOS/nixpkgs/24.05";
    # ...
  }
```

4. Run `nix flake update nixpkgs`.
1. Update the `Cargo.toml` package with the appropriate versioning information. The `version` attribute should include the `nixpkgs` tag,
   and the `rust-version` should be locked at the version of rust which will make the flake checks compile successfully. For `nixpkgs-24.05` that looks like:

```diff
  # Cargo.toml
  [package]
  name = "system_manager"
- version = "0.1.0"
+ version = "0.1.0+nixpkgs-24.05"
+ rust-version = "1.77"
```

6. Run `cargo generate-lockfile`.
1. Ensure the flake checks pass: `nix flake check --keep-going -L`.

> Note that there may be breaking changes between nixpkgs versions which could require additional debugging.

8. Lastly, reference the release branch in your flake:

```diff
  # your-flake.nix
  {
-   inputs.nixpkgs.url = "github:NixOS/nixpkgs/23.11";
+   inputs.nixpkgs.url = "github:NixOS/nixpkgs/24.05";
    inputs.system-manager = {
-     url = "github:numtide/system-manager";
+     type = "github";
+     owner = "numtide";
+     repo = "system-manager";
+     ref = "release-24.05";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  }
```

# Adding New Distributions

This section assumes you have an existing nix installation. Otherwise, refer to the project README for installation instructions.

To add a new distribution, first initialize a new flake repository with `nix run 'github:numtide/system-manager' -- init --flake --allow-any-distro`.
Once the flake is initialized, switch to the new configuration with `nix run 'github:numtide/system-manager' -- switch --flake '.'`. It is not unlikely
that this will produce errors, but please refer to the FAQ, github issues, discussion or contributing guide for potential solutions, or instructions for upstreaming changes.

Once the distribution is stable, it can be added to the `supportedIds` list that is part of the [system-manager module](../../../nix/modules/default.nix)'s `config` attribute.

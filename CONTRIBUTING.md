# Contributing

Welcome! Thank you for your interest in the System Manager project. Your contributions are greatly appreciated.

## Getting Started

System Manager development requires a Nix installation with the `flakes` and `nix-command` features enabled. If you do not have Nix installed, please refer to the [Installation Guide](https://system-manager.net/main/how-to/install/).

1. [Create a fork of the repository](https://github.com/numtide/system-manager/fork)
2. Clone your fork locally:
   ```sh
   git clone git@github.com:<USER>/system-manager.git
   ```
3. Enter the development environment:
   ```sh
   nix develop
   ```
   This provides all tools necessary to build and test the repository.
4. [Create an issue](#creating-issues) for the problem you are trying to solve, if one does not already exist.
5. [Create a pull request](#creating-pull-requests) to close the issue.

## Creating Issues

Before creating a new issue, please [search existing issues](https://github.com/numtide/system-manager/issues) to ensure the problem is not already being tracked.

## Creating Pull Requests

> **Important**: Please ensure an issue exists for the problem you are fixing before opening a pull request.

1. Create a working branch targeting the issue number:
   ```sh
   git checkout -b <USER>/123
   ```
2. Add, commit, and push your changes:
   ```sh
   git add -A
   git commit -m "fix: Fixes ..."
   git push origin <USER>/123
   ```
3. [Open a pull request](https://github.com/numtide/system-manager/compare) targeting the `main` branch.
4. Add a few sentences describing your changes and use [closing keywords](https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/linking-a-pull-request-to-an-issue) to automatically close the related issue.

---

# Extending System Manager

## Adding New Distributions

System Manager officially supports Ubuntu and NixOS. To add support for another distribution:

1. Initialize a new flake with distribution checks disabled:
   ```sh
   nix run 'github:numtide/system-manager' -- init --flake --allow-any-distro
   ```

2. Switch to the new configuration:
   ```sh
   nix run 'github:numtide/system-manager' -- switch --flake '.'
   ```

3. Debug any errors that occur. Refer to the FAQ, GitHub issues, or open a discussion for help.

4. Once stable, the distribution can be added to the `supportedIds` list in the [system-manager module](./nix/modules/default.nix).

## Creating an Ad-Hoc Release

There is currently no release plan that follows nixpkgs releases, but ad-hoc releases are possible. Here's how to create a release branch for a specific nixpkgs version (e.g., `nixpkgs-24.05`):

1. Check that a release branch for the required version does not already exist.

2. Fork and clone the repository, then create a new branch:
   ```sh
   git checkout -b release-24.05
   ```

3. Update the nixpkgs ref in `flake.nix`:
   ```diff
   - inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
   + inputs.nixpkgs.url = "github:NixOS/nixpkgs/24.05";
   ```

4. Update the flake lock:
   ```sh
   nix flake update nixpkgs
   ```

5. Update `Cargo.toml` with version information:
   ```diff
   [package]
   name = "system_manager"
   - version = "0.1.0"
   + version = "0.1.0+nixpkgs-24.05"
   + rust-version = "1.77"
   ```

6. Regenerate the Cargo lock:
   ```sh
   cargo generate-lockfile
   ```

7. Ensure flake checks pass:
   ```sh
   nix flake check --keep-going -L
   ```

   > Note: There may be breaking changes between nixpkgs versions requiring additional debugging.

8. Reference the release branch in your flake:
   ```nix
   {
     inputs = {
       nixpkgs.url = "github:NixOS/nixpkgs/24.05";
       system-manager = {
         type = "github";
         owner = "numtide";
         repo = "system-manager";
         ref = "release-24.05";
         inputs.nixpkgs.follows = "nixpkgs";
       };
     };
   }
   ```

---

# Getting Help

- [GitHub Issues](https://github.com/numtide/system-manager/issues) - Bug reports and feature requests
- [GitHub Discussions](https://github.com/numtide/system-manager/discussions) - Questions and community support
- [Element Chat](https://app.element.io/#/room/#home:numtide.com) - Real-time chat with the Numtide team

# Installation

## Supported Systems

System Manager is in the early stages of development and while it aims to support any Linux
distribution, currently official support is limited to the following distributions:

- Ubuntu
- NixOS

> We would love to expand this list! Please see our [Contributing Guide](./contributing.md)
> for instructions on how to contribute to `system-manager`.

## [NixOS Installation](./installation/nixos.md)

NixOS is a fully reproducible and immutable Linux distribution based on the Nix package manager that uses an atomic update model.

To install System Manager on NixOS, see [NixOS Installation](./installation/nixos.md).

## Other Distributions

### Install Nix

In order to use System Manager, you will first need to install Nix.
You can either use your distro's package manager, or use one of the available options
to install Nix.

- [Official Nix Installer][official-installer] - The canonical source for installing nix.
- [Determinate Nix Installer][detsys-installer] - A wrapper around the official installer that has SELinux support, and enables flake features by default.

> Note: Be advised that the Determinate Systems installer has the option for the official
> Nix, as well as Determinate's own variant of Nix (Determinate Nix). It will prompt you
> for which one you want to install. System Manager is not tested against Determinate Nix.
> It's recommended to use the official Nix if installing via the Determinate Nix Installer.

### [Install System Manager](./installation/standalone.md)

To install System Manager, please find the instructions for your preferred distribution at [Standalone Installation](./installation/standalone.md).

[detsys-installer]: https://github.com/DeterminateSystems/nix-installer
[official-installer]: https://nixos.org/download.html

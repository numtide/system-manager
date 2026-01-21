# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Support running system-manager from macOS to deploy configurations to Linux (#325)

### Fixed

- Remove eval warnings (#324)

### Documentation

- Reorganize documentation with tutorials, how-to guides, explanations, and reference pages
- Add CONTRIBUTING.md with developer guidelines
- Add supported platforms reference page
- Add file permissions and ownership reference

## [1.0.0] - 2026-01-06

### Added

- Init subcommand for initializing system-manager configurations (#210)
- Automated NixOS module compatibility testing tools
- Cachix substituter configured in nix config (#280)
- Nix settings configuration support (#257)
- Manual documentation with mdBook (#206)
- Support for implicit `systemConfigs.${currentSystem}.*` paths (#235)
- Support for attribute sets with string keys (#220)
- File ownership (uid/gid) support for `/etc` files (#191, #192)
- Support for `nixpkgs.config` configuration (#164)
- Support for `buildPlatform`, `hostPlatform`, and overlays (#184)
- Support for `systemd.tmpfiles.settings` (#148)
- Overlay for easier integration with other projects (#125)
- SELinux support documentation
- `allowAnyDistro` option for unsupported distributions (#85)
- Debug output showing nix commands being run
- Remote deployment via `--target-host` option
- System activation and deactivation scripts
- State file for tracking generations
- Assertions support for pre-activation checks

### Fixed

- Return an error if the activation of tmp files fails (#255)
- Cross-compilation issues with makeBinaryWrapper (#229, #234)
- Pass hostname as a quoted string (#243)
- Fix `passwd --stdin` not available on old Ubuntu versions
- Pre-populate script name (#99)
- Use `types.attrs` instead of nonexistent `types.freeform` (#53)
- Adapted systemd module after upstream shellcheck changes
- Switched to nixfmt for code formatting
- Improved CLI API with better subcommands
- Store profile in `/nix/var/nix/profiles` subdirectory
- Refactored systemd activation logic using DBus

### Security

- Avoid unmanaged file overwrites by checking if existing files are managed

### Core Features Implemented

- Configuration of files under `/etc` with proper state tracking
- systemd service management with DBus integration
- systemd tmpfiles.d support (#27)
- Flake-based configuration system
- Generation management with GC root registration
- Activation/deactivation lifecycle management
- Remote deployment support

### Contributors

Thanks to all the contributors who made this release possible:

- Aaron Andersen
- Adrian Hesketh
- Alix Brunet
- bryango
- commiterate
- eureka-cpu
- ginkogruen
- Jean-François Roche
- Jeffrey Freckleface Cogswell
- Jonas Chevalier
- Julien Malka
- Michal Sojka
- Mike Lloyd
- mjones-vsat
- Nick Curran
- Nikolay Yakimov
- oluceps
- Phani Rithvij
- Pierre-Etienne Meunier
- Ramses
- Silver
- Sofie
- Steve Dodd
- Yvan Sraka

[unreleased]: https://github.com/numtide/system-manager/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/numtide/system-manager/releases/tag/v1.0.0

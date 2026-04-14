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

System Manager officially supports Ubuntu, Debian, and NixOS.
Promoting a new distribution to officially-supported status means it is exercised by CI on every PR and a regression in it blocks the build.

### Trying a distribution informally

If you just want to run System Manager on an untested distribution without contributing it back, initialize a flake and disable the OS check by setting `system-manager.allowAnyDistro = true` in your configuration module:

```nix
{
  config.system-manager.allowAnyDistro = true;
}
```

Then iterate with `nix run 'github:numtide/system-manager' -- switch --flake '.'` and debug any errors using the FAQ, GitHub issues, or a discussion.
Once the distribution is stable for your use case, consider upstreaming it via the steps below.

### Adding official support

Adding a distribution touches four areas: the OS allow-list, the container test driver, the VM test driver, and the documentation.

**1. Add the distribution ID to the OS allow-list.**
Edit `nix/modules/default.nix` and append the `/etc/os-release` `ID` value to the `supportedIds` list inside the `osVersion` pre-activation assertion.
The check is bypassed when users set `system-manager.allowAnyDistro = true`, but the allow-list is what controls the default.

**2. Add a container test entry.**
Container tests live under `testFlake/container-tests/` and are parameterized over every distribution declared in `lib/container-test-driver/distros.nix`.
Adding a new entry there causes all existing tests to automatically generate a `container-<distro>-*` variant via `forEachDistro`.

The entry must supply `systems`, a `rootfs` derivation built by `lib.container-test-driver.make-rootfs.buildRootfs`, and a `maskableService` (a systemd unit that test scripts may mask, typically `unattended-upgrades.service` or equivalent).

`buildRootfs` accepts three upstream image formats via `cloudImgFormat`, and the right choice depends on what the distribution publishes:

- `"tar"` (default) consumes a flat rootfs tarball such as Ubuntu's `*-server-cloudimg-amd64-root.tar.xz`. This is the simplest path, has no architecture restrictions, and should be preferred whenever the distribution ships a rootfs tarball.
- `"disk-tarball"` consumes a `.tar.xz` that wraps a raw disk image, such as Debian's `*-genericcloud-*.tar.xz`. It unpacks the outer tarball, locates the root partition with `sfdisk -J` + `jq`, extracts it with `dd`, and dumps the ext4 filesystem into `$out` via `debugfs -R "rdump / $out"`. All required tools (`util-linux`, `e2fsprogs`, `jq`) are cross-architecture in nixpkgs, so this works on both `x86_64-linux` and `aarch64-linux`. `excludePatterns` are applied as a post-extraction prune pass rather than as tar `--exclude` flags. Note: this currently assumes the root filesystem is ext4; a btrfs-backed rootfs (such as Fedora Workstation) would need a `btrfs restore`-based variant added alongside.
- `"qcow2"` extracts the rootfs from a qcow2 cloud disk image using `guestfish tar-out`. It pulls in `libguestfs-with-appliance`, whose `libguestfs-appliance` subpackage is marked `meta.platforms = [ "i686-linux" "x86_64-linux" ]` in nixpkgs, so entries using this format must restrict `systems` to `x86_64-linux`. Use only as a last resort, when the distribution publishes neither a rootfs tarball nor a disk-in-tarball variant.

Pin a specific dated build directory upstream rather than `latest/` and obtain the SHA256 with `nix-prefetch-url`. URL and hash go in `lib/container-test-driver/images.json`; `distros.nix` reads them automatically.

Reuse the existing `excludePatterns` (which strip container-incompatible systemd units) and `extraDirs` (per-package-manager directories like `var/lib/apt/lists/partial`) as a starting point and trim or extend them based on the first build.

**3. Add a VM test entry.**
VM tests live under `testFlake/vm-tests/` and iterate over distributions exposed by `nix-vm-test`.
Edit the `distros` attrset in `testFlake/vm-tests/default.nix` to add a key matching the `nix-vm-test` distribution name (`ubuntu`, `debian`, `fedora`, `rocky`).
Each entry takes a `filter` predicate that selects which versions to exercise — use it to skip versions you do not want in the matrix.
If `nix-vm-test` does not yet support the distribution, support must be added there first.

**4. Run the test matrix and triage failures.**
Build the new check attributes via `nix build .#checks.x86_64-linux.container-<distro>-*` and `vm-<distro>-*-*` and triage any failures.
Prefer fixing tests to be distribution-agnostic over skipping them.

**5. Update documentation.**
The user-facing platform statement lives in `docs/site/reference/supported-platforms.md`, with secondary mentions in `README.md`, `docs/site/how-to/install.md`, `docs/site/tutorials/getting-started.md`, and `docs/site/how-to/test-configuration.md`.
Mention the new distribution alongside the existing supported ones.

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

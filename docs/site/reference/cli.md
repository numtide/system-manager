# CLI Commands

The basic command looks like this:

```sh
nix run 'github:numtide/system-manager' -- switch --flake . --sudo
```

This is the most common scenario you'll use.

## Command-line Options

### init

This subcommand creates two initial files for use with System Manager, a fully-functional `flake.nix`, and a `system.nix` file that contains skeleton code.

#### Command line options

**path:** The path where to create the files. If the path doesn't exist, it will be created.

#### Example

```sh
nix run 'github:numtide/system-manager' -- init
```

This will create the initial files in `~/.config/system-manager`.

```sh
nix run 'github:numtide/system-manager' -- init --path='/home/ubuntu/system-manager'
```

This will create the initial files in `/home/ubuntu/system-manager`.

!!! Note
    System Manager requires flakes to be enabled in `/etc/nix/nix.conf`. See [Enabling Flakes](../how-to/install.md#enabling-flakes) for setup instructions.

### switch

The `switch` subcommand builds and activates your configuration immediately, making it both the current running configuration and the default for future boots. Use it whenever you want to apply your changes.

**Note: Rollbacks are not yet implemented.**

The following two parameters are currently both required:

**--flake**: Specifies a flake to use for configuration.

**--sudo**: Specifies that System Manager can use sudo.

### register

The `register` subcommand builds and registers a System Manager configuration, but does not activate it. Compare this to `switch`, which does everything register does, but then activates it.

### build

The `build` subcommand builds everything needed for a switch, but does not register it.

### deactivate

The `deactivate` deactivates System Manager.

### pre-populate

The `pre-populate` subcommand puts all files defined by the given generation in place, but does not start the services. This is useful in scripts.

### sudo

The `sudo` subcommand grants sudo access to System Manager, while running under the current user. All created files will be owned by the current user.

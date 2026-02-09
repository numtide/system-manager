# Managing pre-existing files

System-manager is designed for non-NixOS Linux distributions that already have populated `/etc` directories.
By default, when it encounters a file at a target path that it didn't create, it refuses to overwrite it and logs an error.
This page explains how to handle these conflicts.

## The problem

On a fresh Ubuntu or Debian system, files like `/etc/nix/nix.conf` (created by the Nix installer) or systemd timer symlinks in `.wants` directories already exist.
When system-manager tries to manage these paths, activation skips them:

```
Unmanaged path already exists in filesystem, please remove it and run system-manager again: /etc/nix/nix.conf
```

The activation continues but the conflicting entries are not applied.

## Replacing individual etc files

Use `replaceExisting = true` on any `environment.etc` entry to have system-manager back up the existing file before replacing it.
On deactivation, the original is restored.

```nix
{ ... }:
{
  environment.etc."my-app/config.toml" = {
    text = ''
      [server]
      port = 8080
    '';
    mode = "0644";
    replaceExisting = true;
  };
}
```

During activation, the pre-existing file is renamed to `/etc/my-app/config.toml.system-manager-backup`.
When system-manager is deactivated or the entry is removed from the configuration, the backup is restored to its original path.

## Nix configuration

The `nix` module is enabled by default and generates `/etc/nix/nix.conf` from `nix.settings`.
Since the Nix installer already creates this file, you need `replaceExisting`:

```nix
{ ... }:
{
  nix.settings = {
    experimental-features = [ "nix-command" "flakes" ];
    trusted-users = [ "myuser" ];
  };

  environment.etc."nix/nix.conf".replaceExisting = true;
}
```

This backs up the installer-created `nix.conf` and replaces it with the one generated from `nix.settings`.
On deactivation, the original is restored so `nix` keeps working.

## Systemd timer and service conflicts

Systemd `.wants` and `.requires` directories are handled automatically.
When system-manager declares a timer with `wantedBy` and the target `.wants` directory already contains a symlink for that unit (common on Ubuntu/Debian), the existing symlink is backed up and replaced without requiring any configuration.

```nix
{ pkgs, ... }:
{
  systemd.timers.logrotate = {
    wantedBy = [ "timers.target" ];
    timerConfig = {
      OnCalendar = "*:0/5";
      Persistent = true;
    };
  };

  systemd.services.logrotate = {
    serviceConfig.Type = "oneshot";
    script = "${pkgs.logrotate}/bin/logrotate /etc/logrotate.conf";
  };
}
```

If `/etc/systemd/system/timers.target.wants/logrotate.timer` already exists (Ubuntu pre-installs it), system-manager backs it up and creates its own symlink.
Other entries in the `.wants` directory that system-manager does not manage are left untouched.
On deactivation, the original symlink is restored.

## How backups work

Backups are stored next to the original file with a `.system-manager-backup` suffix.
The file tree state tracks which paths have backups via a `ManagedWithBackup` status, so deactivation knows to restore them rather than simply deleting the managed file.

| Event | Action |
|-------|--------|
| Activation with `replaceExisting` | Rename existing file to `<path>.system-manager-backup`, create managed entry |
| Activation of `.wants`/`.requires` entry | Same, automatically |
| Re-activation (same config) | No change, symlink already up to date |
| Deactivation | Remove managed entry, rename backup back to original path |

## See also

- [Getting Started](../tutorials/getting-started.md) for initial setup
- [Timer example](../examples/timer.md) for systemd timer configuration
- [Rollback Changes](rollback.md) for reverting to previous configurations

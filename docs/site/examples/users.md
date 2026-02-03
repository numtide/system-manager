# Users and groups

This example demonstrates how to declaratively manage users and groups with System Manager.

## Configuration

### system.nix

```nix
{ pkgs, ... }:
{
  nixpkgs.hostPlatform = "x86_64-linux";

  # Create a normal user account
  users.users.alice = {
    isNormalUser = true;
    description = "Alice User";
    extraGroups = [ "wheel" "docker" ];
    # Set an initial password (only applied on first creation if mutableUsers = true)
    initialPassword = "changeme";
  };

  # Create a system user for running services
  users.users.myapp = {
    isSystemUser = true;
    group = "myapp";
    home = "/var/lib/myapp";
    createHome = true;
    description = "My Application service account";
  };

  # Create the group for the system user
  users.groups.myapp = {};

  # Create additional groups
  users.groups.docker = {};
}
```

## User types

System Manager distinguishes between two types of users, and exactly one must be specified.

*Normal users* are interactive accounts for people logging into the system.
Setting `isNormalUser = true` automatically configures sensible defaults: a home directory at `/home/<username>`, membership in the `users` group, and the default shell.

*System users* are non-interactive accounts for running services.
Setting `isSystemUser = true` creates an account with a UID below 1000 and no login shell by default.
System users require an explicit `group` setting.

## Password options

Several options control user passwords.
For systems where `users.mutableUsers = true` (the default), use `initialPassword` or `initialHashedPassword` to set a password only when the user is first created.
Users can then change their password with `passwd`.

For immutable configurations where `users.mutableUsers = false`, use `hashedPassword` or `hashedPasswordFile` to enforce a specific password on every activation.

Generate a hashed password with `mkpasswd`:

```bash
mkpasswd -m sha-512
```

## Advanced example

This configuration shows additional options for user management:

```nix
{ pkgs, ... }:
{
  nixpkgs.hostPlatform = "x86_64-linux";

  # Disable mutable users for fully declarative management
  # users.mutableUsers = false;

  users.users.bob = {
    isNormalUser = true;
    description = "Bob Developer";
    home = "/home/bob";
    shell = pkgs.zsh;
    extraGroups = [ "wheel" "networkmanager" ];
    # Hashed password (use mkpasswd to generate)
    hashedPassword = "$6$rounds=500000$example$hashedpasswordhere";
    # Or read from a file at activation time
    # hashedPasswordFile = "/run/secrets/bob-password";
  };

  users.users.postgres = {
    isSystemUser = true;
    group = "postgres";
    home = "/var/lib/postgresql";
    createHome = true;
    description = "PostgreSQL server";
  };

  users.groups.postgres = {};
}
```

## What this configuration does

1. Creates user accounts in `/etc/passwd` and `/etc/shadow`
2. Creates groups in `/etc/group`
3. Sets up home directories when `createHome = true`
4. Manages subordinate UID/GID ranges in `/etc/subuid` and `/etc/subgid` for container support
5. Preserves existing passwords and UIDs when `mutableUsers = true`

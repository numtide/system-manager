# Module Options Reference

This reference documents all configuration options available in System Manager modules.

## nixpkgs

Options for configuring the Nix package set.

### nixpkgs.hostPlatform

**Type:** `string` or `attrset`

**Default:** (required)

**Example:** `"x86_64-linux"`

The platform for which packages are built. This option must be set in every configuration.

### nixpkgs.buildPlatform

**Type:** `string`

**Default:** Value of `nixpkgs.hostPlatform`

**Example:** `"x86_64-linux"`

The platform on which packages are built. Used for cross-compilation.

### nixpkgs.overlays

**Type:** `list of overlays`

**Default:** `[]`

Overlays to apply to the package set.

### nixpkgs.config

**Type:** `attrset`

**Default:** `{}`

Configuration passed to nixpkgs when instantiating the package set (e.g., `{ allowUnfree = true; }`).

---

## environment

Options for system environment configuration.

### environment.systemPackages

**Type:** `list of packages`

**Default:** `[]`

Packages to install system-wide. Installed packages are available at `/run/system-manager/sw/bin/`.

```nix
environment.systemPackages = with pkgs; [
  vim
  git
  htop
];
```

### environment.pathsToLink

**Type:** `list of strings`

**Default:** `[ "/bin" ]`

Paths to link from packages into the system environment.

---

## environment.etc

Manages files under `/etc`. Each attribute creates a file at `/etc/<name>`.

**Type:** `attrset of submodules`

**Default:** `{}`

```nix
environment.etc."myapp/config.conf" = {
  text = "setting = value";
  mode = "0644";
};
```

### environment.etc.{name}.enable

**Type:** `boolean`

**Default:** `true`

Whether to create this file.

### environment.etc.{name}.text

**Type:** `null or string`

**Default:** `null`

Content of the file. Mutually exclusive with `source`.

### environment.etc.{name}.source

**Type:** `path`

**Default:** Derived from `text` if set

Path to the source file.

### environment.etc.{name}.target

**Type:** `string`

**Default:** Attribute name

Target path relative to `/etc`.

### environment.etc.{name}.mode

**Type:** `string`

**Default:** `"symlink"`

**Example:** `"0644"`

File mode. Use `"symlink"` to create a symlink to the Nix store, or an octal mode (e.g., `"0644"`) to copy the file with that mode.

### environment.etc.{name}.uid

**Type:** `integer`

**Default:** `0`

Numeric user ID for file ownership. Only applies when `mode` is not `"symlink"`.

### environment.etc.{name}.gid

**Type:** `integer`

**Default:** `0`

Numeric group ID for file ownership. Only applies when `mode` is not `"symlink"`.

### environment.etc.{name}.user

**Type:** `string`

**Default:** `"+<uid>"`

User name for file ownership. Takes precedence over `uid`. Only applies when `mode` is not `"symlink"`.

### environment.etc.{name}.group

**Type:** `string`

**Default:** `"+<gid>"`

Group name for file ownership. Takes precedence over `gid`. Only applies when `mode` is not `"symlink"`.

---

## systemd

Options for systemd unit management.

### systemd.package

**Type:** `package`

**Default:** `pkgs.systemdMinimal`

The systemd package to use.

### systemd.globalEnvironment

**Type:** `attrset of (null or string or path or package)`

**Default:** `{}`

**Example:** `{ TZ = "CET"; }`

Environment variables passed to all systemd units.

### systemd.enableStrictShellChecks

**Type:** `boolean`

**Default:** `false`

Run shellcheck on generated unit scripts.

---

## systemd.services

Defines systemd service units.

**Type:** `attrset of service submodules`

**Default:** `{}`

```nix
systemd.services.myservice = {
  description = "My Service";
  wantedBy = [ "system-manager.target" ];
  serviceConfig = {
    Type = "oneshot";
    ExecStart = "${pkgs.hello}/bin/hello";
  };
};
```

Common service options:

| Option | Type | Description |
|--------|------|-------------|
| `enable` | boolean | Whether to enable this service (default: `true`) |
| `description` | string | Service description |
| `wantedBy` | list of strings | Targets that want this service |
| `after` | list of strings | Units this service starts after |
| `requires` | list of strings | Units this service requires |
| `serviceConfig` | attrset | systemd `[Service]` section options |
| `script` | string | Shell script to execute |
| `environment` | attrset | Environment variables for this service |
| `path` | list of packages | Packages to add to `PATH` |

!!! tip "Starting services on activation"
    Use `wantedBy = [ "system-manager.target" ];` to start a service when System Manager activates.

---

## systemd.timers

Defines systemd timer units for scheduled tasks.

**Type:** `attrset of timer submodules`

**Default:** `{}`

```nix
systemd.timers.mytimer = {
  wantedBy = [ "timers.target" ];
  timerConfig = {
    OnCalendar = "daily";
    Persistent = true;
  };
};
```

Common timer options:

| Option | Type | Description |
|--------|------|-------------|
| `enable` | boolean | Whether to enable this timer |
| `description` | string | Timer description |
| `wantedBy` | list of strings | Targets that want this timer |
| `timerConfig` | attrset | systemd `[Timer]` section options |

---

## systemd.sockets

Defines systemd socket units for socket activation.

**Type:** `attrset of socket submodules`

**Default:** `{}`

---

## systemd.targets

Defines systemd target units.

**Type:** `attrset of target submodules`

**Default:** `{}`

!!! note
    System Manager provides a `system-manager.target` that is wanted by `default.target`.

---

## systemd.paths

Defines systemd path units for path-based activation.

**Type:** `attrset of path submodules`

**Default:** `{}`

---

## systemd.mounts

Defines systemd mount units.

**Type:** `list of mount submodules`

**Default:** `[]`

!!! note
    This is a list (not attrset) because systemd requires mount unit names to match the mount path.

---

## systemd.automounts

Defines systemd automount units.

**Type:** `list of automount submodules`

**Default:** `[]`

---

## systemd.slices

Defines systemd slice units for resource management.

**Type:** `attrset of slice submodules`

**Default:** `{}`

---

## systemd.generators

Defines systemd generators.

**Type:** `attrset of paths`

**Default:** `{}`

**Example:** `{ systemd-gpt-auto-generator = "/dev/null"; }`

Creates symlinks from `/etc/systemd/system-generators/<name>` to the specified path.

---

## systemd.shutdown

Defines systemd shutdown executables.

**Type:** `attrset of paths`

**Default:** `{}`

Creates symlinks from `/etc/systemd/system-shutdown/<name>` to the specified path.

---

## systemd.tmpfiles

Manages temporary files and directories via systemd-tmpfiles.

### systemd.tmpfiles.rules

**Type:** `list of strings`

**Default:** `[]`

**Example:** `[ "d /tmp 1777 root root 10d" ]`

Rules in tmpfiles.d(5) format.

### systemd.tmpfiles.settings

**Type:** `nested attrset`

**Default:** `{}`

Structured tmpfiles configuration.

```nix
systemd.tmpfiles.settings."10-myapp" = {
  "/var/lib/myapp".d = {
    mode = "0755";
    user = "root";
    group = "root";
  };
};
```

Each entry supports:

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `type` | string | Key name | Operation type (see tmpfiles.d(5)) |
| `mode` | string | `"-"` | File mode |
| `user` | string | `"-"` | Owner user |
| `group` | string | `"-"` | Owner group |
| `age` | string | `"-"` | Cleanup age |
| `argument` | string | `""` | Type-specific argument |

### systemd.tmpfiles.packages

**Type:** `list of packages`

**Default:** `[]`

Packages containing tmpfiles.d rules in `lib/tmpfiles.d/*.conf`.

---

## networking

### networking.enableIPv6

**Type:** `boolean`

**Default:** `true`

Whether to enable IPv6 support.

---

## system-manager

### system-manager.allowAnyDistro

**Type:** `boolean`

**Default:** `false`

Bypass distribution compatibility checks. Enable this to use System Manager on untested distributions.

---

## See Also

- [Getting Started Tutorial](../tutorials/getting-started.md) - Learn module basics
- [First Service Tutorial](../tutorials/first-service.md) - Create your first systemd service
- [systemd.exec(5)](https://www.freedesktop.org/software/systemd/man/systemd.exec.html) - Service execution options
- [tmpfiles.d(5)](https://www.freedesktop.org/software/systemd/man/tmpfiles.d.html) - Tmpfiles format

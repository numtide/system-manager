# Import an upstream NixOS module

System Manager ships with a curated set of NixOS modules (nginx, ACME, userborn, nix settings), but the NixOS module library contains hundreds more.
You can import additional NixOS modules into your system-manager configuration by referencing them through the `nixosModulesPath` special argument and providing stubs for any options the module expects but that system-manager does not define.

This guide walks through the technique using real patterns from the system-manager codebase.

## How NixOS modules are loaded

When you call `makeSystemConfig`, system-manager passes `nixosModulesPath` as a special argument available to all modules.
This path points to the NixOS modules directory inside nixpkgs (`<nixpkgs>/nixos/modules`), giving you access to the entire NixOS module library.

An upstream NixOS module is imported by mapping its path relative to that directory:

```nix
{ nixosModulesPath, ... }:
{
  imports = [
    (nixosModulesPath + "/services/monitoring/prometheus/exporters.nix")
  ];
}
```

In practice, most NixOS modules reference options defined by other NixOS modules that system-manager does not include.
When the module system encounters an undefined option, evaluation fails with an error like:

```
error: The option `boot.kernel.sysctl' does not exist.
```

The solution is to declare *stub options* that satisfy the module's references without implementing the underlying functionality.

## Creating stub options

A stub option declares an option name with a type and default value so that module evaluation succeeds.
The simplest form uses `lib.types.raw`, which accepts any value without validation:

```nix
{ lib, ... }:
{
  options.boot = lib.mkOption {
    type = lib.types.raw;
  };
}
```

This is the pattern system-manager uses internally for `boot.*`, which many NixOS modules reference but which has no meaning outside NixOS.

When you need the stub to carry a specific default value (for example, an empty list or `false`), use a concrete type:

```nix
{ lib, ... }:
{
  options.services.openssh = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
    };
    hostKeys = lib.mkOption {
      type = lib.types.listOf (
        lib.types.submodule {
          options = {
            path = lib.mkOption { type = lib.types.path; };
            type = lib.mkOption { type = lib.types.str; };
          };
        }
      );
      default = [ ];
    };
  };
}
```

This is the actual stub that system-manager uses to allow sops-nix to evaluate.
The sops-nix module reads `services.openssh.hostKeys` to auto-detect SSH keys for age decryption; the empty default means no keys are auto-detected, and users set `sops.age.sshKeyPaths` explicitly instead.

## Adapting service targets

NixOS services typically declare `wantedBy = [ "multi-user.target" ]`.
In system-manager, services must be wanted by `system-manager.target` to start during activation.
Override this with `lib.mkForce`:

```nix
{ config, lib, ... }:
{
  systemd.services.myservice = lib.mkIf config.services.myservice.enable {
    wantedBy = lib.mkForce [ "system-manager.target" ];
  };
}
```

This is exactly how the built-in nginx adapter works: it imports the upstream NixOS nginx module, then overrides the target in a local wrapper.

## Step-by-step walkthrough

This walkthrough imports the saslauthd module (Cyrus SASL authentication daemon) from nixpkgs.
It is a self-contained 58-line module that defines a single systemd service with no external NixOS module dependencies, making it a straightforward candidate.

### 1. Try to build

Create a module that imports saslauthd and adapts its service target:

```nix
# saslauthd.nix
{ nixosModulesPath, config, lib, ... }:
{
  imports = [
    (nixosModulesPath + "/services/system/saslauthd.nix")
  ];

  systemd.services.saslauthd = lib.mkIf config.services.saslauthd.enable {
    wantedBy = lib.mkForce [ "system-manager.target" ];
  };
}
```

Add it to your `makeSystemConfig` modules list:

```nix
# flake.nix (relevant excerpt)
systemConfigs.default = system-manager.lib.makeSystemConfig {
  modules = [
    ./saslauthd.nix
    {
      nixpkgs.hostPlatform = "x86_64-linux";
      services.saslauthd.enable = true;
    }
  ];
};
```

Then build:

```bash
nix build .#systemConfigs.default
```

Because this module only uses `systemd.services`, which system-manager already provides, no stubs are needed and the build succeeds.

### 2. When stubs are needed

If the build had failed with an error like:

```
error: The option `some.nixos.option' does not exist.
```

you would add stub options to the same module (or a separate file) to satisfy the reference, as described in [creating stub options](#creating-stub-options) above.

### 3. Configure the service

The upstream module options are now available in your configuration:

```nix
{
  services.saslauthd = {
    enable = true;
    mechanism = "pam";
    config = ''
      # saslauthd configuration
    '';
  };
}
```

### 4. Verify

Build the configuration to confirm it evaluates without errors:

```bash
nix build .#systemConfigs.default
```

## Real-world example: sops-nix stubs

The system-manager codebase provides a concrete reference for this technique.
The sops-nix module uses NixOS activation scripts, which system-manager does not support.
To allow the module to evaluate, system-manager declares stub options in `nix/modules/upstream/sops-nix.nix`:

```nix
{ lib, ... }:
{
  options.system.activationScripts = {
    generate-age-key = lib.mkOption {
      type = lib.types.raw;
      default = "";
    };
    setupSecrets = lib.mkOption {
      type = lib.types.raw;
      default = "";
    };
    setupSecretsForUsers = lib.mkOption {
      type = lib.types.raw;
      default = "";
    };
  };
}
```

These stubs absorb the values that sops-nix writes to activation scripts without executing them.
The actual secret decryption is handled differently in system-manager through a systemd service.

## Tips

Not every NixOS module can work with system-manager.
Modules that depend on kernel features (`boot.*`), the NixOS activation system, or NixOS-specific infrastructure like the NixOS module evaluator may require more extensive stubs or may not be practical to port.
Services that only need systemd units, `/etc` files, and packages are the best candidates.

When evaluating whether a module is worth importing, read its source in nixpkgs to understand its dependencies.
A module that primarily generates a systemd service and a configuration file is straightforward to adapt.
A module that deeply integrates with the NixOS activation system or requires multiple other NixOS modules will need proportionally more stubs.

## See also

- [Module options reference](../reference/modules.md) for options already available in system-manager
- [NixOS comparison](../explanation/nixos-comparison.md) for differences between NixOS and system-manager
- [Test configuration](test-configuration.md) for validating your configuration with container tests

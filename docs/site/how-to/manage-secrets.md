# Manage secrets with sops-nix

[sops-nix](https://github.com/Mic92/sops-nix) decrypts secrets that are committed to your repository in encrypted form and installs them, at runtime, into a `ramfs` under `/run/secrets`.
System Manager supports it, so encrypted secrets can live in Git while the plaintext never touches disk.

## How it works here

On NixOS, sops-nix normally installs secrets from an activation script.
System Manager does not run NixOS activation scripts: it activates a configuration by writing systemd units and restarting `sysinit-reactivation.target`.

Instead, System Manager uses the systemd code path that sops-nix already provides (`sops.useSystemdActivation`), which installs secrets from a `sops-install-secrets.service` oneshot unit.
This is enabled by default and is ordered so that secrets are installed both on boot and on every `system-manager switch`.

## Basic usage

Add sops-nix as a flake input and import its NixOS module:

```nix
{
  inputs.sops-nix.url = "github:Mic92/sops-nix";
  inputs.sops-nix.inputs.nixpkgs.follows = "nixpkgs";
}
```

```nix
{ config, ... }:
{
  imports = [ inputs.sops-nix.nixosModules.sops ];

  sops = {
    age.keyFile = "/var/lib/sops-nix/key.txt";
    defaultSopsFile = ./secrets.yaml;
    secrets.my-service-env = { };
  };

  systemd.services.my-service = {
    wantedBy = [ "system-manager.target" ];
    serviceConfig.EnvironmentFile = config.sops.secrets.my-service-env.path;
  };
}
```

The secret is decrypted into `/run/secrets/my-service-env`, owned by root with mode `0400` by default.

## Choosing a decryption key

Because System Manager does not manage `services.openssh`, sops-nix cannot auto-detect SSH host keys.
Pick one of:

- **An age key file** — set `sops.age.keyFile` to a path outside the Nix store, and provision the key out of band.
- **An SSH host key** — set `sops.age.sshKeyPaths = [ "/etc/ssh/ssh_host_ed25519_key" ];` explicitly to reuse a key the machine already has.
- **A generated age key** — set `sops.age.generateKey = true;` together with `sops.age.keyFile`.
  System Manager generates the key from a `sops-generate-age-key.service` oneshot ordered before secret installation.
  The key is only generated if the file does not exist yet, so you have to add its public half to your `.sops.yaml` and re-encrypt before secrets can be decrypted on that machine.

## Restarting services when a secret changes

`sops.secrets.<name>.restartUnits` and `reloadUnits` work as they do on NixOS: `sops-install-secrets` restarts or reloads the listed units through `systemctl` when the decrypted value changes.

```nix
sops.secrets.my-service-env.restartUnits = [ "my-service.service" ];
```

## Secrets needed to create users

Secrets marked `neededForUsers` are installed into `/run/secrets-for-users` before users are created, which is how you can source a user's password hash from sops:

```nix
{ config, ... }:
{
  sops.secrets.alice-password.neededForUsers = true;

  users.users.alice = {
    isNormalUser = true;
    hashedPasswordFile = config.sops.secrets.alice-password.path;
  };
}
```

This requires `services.userborn.enable` (the default), since userborn is what creates the users.

## Limitations

- `sops.useSystemdActivation` cannot be disabled: the activation-script path it falls back to does not exist in System Manager. Setting it to `false` fails evaluation with an explanatory error.
- Secrets live in `/run` and are re-installed on every boot and every activation. A machine that cannot reach its decryption key will fail `sops-install-secrets.service` rather than start services with stale secrets.

## See also

- [Import a NixOS module](import-nixos-module.md) for the technique behind the sops-nix compatibility module
- [Users and groups example](../examples/users.md)

{
  nixosModulesPath,
  lib,
  ...
}:
{
  imports = [
    ./dhparams.nix
    ./firewall.nix
    ./nginx.nix
    ./nix.nix
    ./programs/ssh.nix
    ./security-wrappers.nix
    ./security/sudo.nix
    ./userborn.nix
    ./users-groups.nix
    ../sops-nix.nix
    ./openssh.nix
  ]
  ++
    # List of imported NixOS modules
    # TODO: how will we manage this in the long term?
    map (path: nixosModulesPath + path) [
      "/misc/meta.nix"
      "/misc/ids.nix"
      "/security/acme/"
      "/security/dhparams.nix"
      "/security/sudo.nix"
      "/security/wrappers/"
      "/services/web-servers/nginx/"
      # nix settings
      "/config/nix.nix"
      "/config/nix-channel.nix"
      "/config/nix-flakes.nix"
      "/services/system/userborn.nix"
      "/system/build.nix"
    ];

  options =
    # We need to ignore a bunch of options that are used in NixOS modules but
    # that don't apply to system-manager configs.
    # TODO: can we print an informational message for things like kernel modules
    # to inform users that they need to be enabled in the host system?
    {
      boot = lib.mkOption {
        type = lib.types.raw;
      };

      # nixos/modules/services/system/userborn.nix still depends on activation scripts
      # but just to verify that the "users" activation script is disabled.
      # We try to avoid having to import the whole activationScripts module.
      system.activationScripts.users = lib.mkOption {
        type = lib.types.str;
        default = "";
      };

      # nix-channel.nix just emits a warning in an activation script.
      # As we don't support NixOS activation scripts, we just ignore it.
      system.activationScripts.no-nix-channel = lib.mkOption {
        type = lib.types.raw;
        default = "";
      };

    };
}

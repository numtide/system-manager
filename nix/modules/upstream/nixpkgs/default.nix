{
  nixosModulesPath,
  lib,
  pkgs,
  config,
  ...
}:
let
  modulesTypeDesc = ''
    This can either be a list of modules, or an attrset. In an
    attrset, names that are set to `true` represent modules that will
    be included. Note that setting these names to `false` does not
    prevent the module from being loaded.
  '';
  kernelModulesConf = pkgs.writeText "nixos.conf" ''
    ${lib.concatStringsSep "\n" config.boot.kernelModules}
  '';
  attrNamesToTrue = lib.types.coercedTo (lib.types.listOf lib.types.str) (
    enabledList: lib.genAttrs enabledList (_attrName: true)
  ) (lib.types.attrsOf lib.types.bool);
in
{
  imports = [
    ./firewall.nix
    ./nginx.nix
    ./nix.nix
    ./security-wrappers.nix
    ./userborn.nix
    ./users-groups.nix
    ../sops-nix.nix
  ]
  ++
    # List of imported NixOS modules
    # TODO: how will we manage this in the long term?
    map (path: nixosModulesPath + path) [
      "/misc/meta.nix"
      "/misc/ids.nix"
      "/security/acme/"
      "/security/wrappers/"
      "/services/web-servers/nginx/"
      "/config/sysctl.nix"
      # nix settings
      "/config/nix.nix"
      "/services/system/userborn.nix"
      "/system/build.nix"
    ];

  options =
    # We need to ignore a bunch of options that are used in NixOS modules but
    # that don't apply to system-manager configs.
    {
      boot = {
        kernelModules = lib.mkOption {
          type = attrNamesToTrue;
          default = { };
          description = ''
            The set of kernel modules to be loaded in the second stage of
            the boot process.

            ${modulesTypeDesc}
          '';
          apply = mods: lib.attrNames (lib.filterAttrs (_: v: v) mods);
        };

        kernelPackages = lib.mkOption {
          type = lib.types.raw;
          default = {
            kernel.version = "stub";
          };
          description = "Stub kernel packages for compatibility; not actively used in system-manager.";
        };
      };

      # nixos/modules/services/system/userborn.nix still depends on activation scripts
      # but just to verify that the "users" activation script is disabled.
      # We try to avoid having to import the whole activationScripts module.
      system.activationScripts.users = lib.mkOption {
        type = lib.types.str;
        default = "";
      };

      services.openssh = {
        enable = lib.mkOption {
          type = lib.types.bool;
          default = false;
        };
        # sops-nix uses hostKeys to find SSH keys for age decryption.
        # On non-NixOS systems, users should set sops.age.sshKeyPaths explicitly.
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
    };

  config = {
    # Create /etc/modules-load.d/system-manager.conf, which is read by
    # systemd-modules-load.service to load required kernel modules.
    environment.etc = lib.mkIf (config.boot.kernelModules != { }) {
      "modules-load.d/system-manager.conf".source = kernelModulesConf;
    };

    systemd.services.systemd-modules-load.overrideStrategy = "asDropin";
    systemd.services.systemd-modules-load = {
      wantedBy = [
        "system-manager.target"
        "multi-user.target"
      ];
      restartTriggers = [ kernelModulesConf ];
      serviceConfig = {
        SuccessExitStatus = "0 1";
      };
    };

    systemd.services.systemd-sysctl.overrideStrategy = "asDropin";
    systemd.services.systemd-sysctl = {
      wantedBy = [
        "system-manager.target"
        "multi-user.target"
      ];
      restartTriggers = [ config.environment.etc."sysctl.d/60-nixos.conf".source ];
    };
  };
}

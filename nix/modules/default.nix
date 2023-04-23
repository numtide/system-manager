{ lib
, config
, ...
}:
{
  imports = [
    ./etc.nix
    ./systemd.nix
    ./upstream/nixpkgs
  ];

  options =
    let
      inherit (lib) types;
    in
    {

      nixpkgs = {
        # TODO: switch to lib.systems.parsedPlatform
        hostPlatform = lib.mkOption {
          type = types.str;
          example = "x86_64-linux";
          default = throw "the option nixpkgs.hostPlatform needs to be set.";
        };
      };

      assertions = lib.mkOption {
        type = types.listOf types.unspecified;
        internal = true;
        default = [ ];
        example = [{ assertion = false; message = "you can't enable this for that reason"; }];
        description = lib.mdDoc ''
          This option allows modules to express conditions that must
          hold for the evaluation of the system configuration to
          succeed, along with associated error messages for the user.
        '';
      };

      warnings = lib.mkOption {
        internal = true;
        default = [ ];
        type = types.listOf types.str;
        example = [ "The `foo' service is deprecated and will go away soon!" ];
        description = lib.mdDoc ''
          This option allows modules to show warnings to users during
          the evaluation of the system configuration.
        '';
      };

      # Statically assigned UIDs and GIDs.
      # Ideally we use DynamicUser as much as possible to avoid the need for these.
      ids = {
        uids = lib.mkOption {
          internal = true;
          description = lib.mdDoc ''
            The user IDs used by system-manager.
          '';
          type = types.attrsOf types.int;
        };

        gids = lib.mkOption {
          internal = true;
          description = lib.mdDoc ''
            The group IDs used by system-manager.
          '';
          type = types.attrsOf types.int;
        };
      };

      # No-op option for now.
      # TODO: should we include the settings in /etc/logrotate.d ?
      services.logrotate = lib.mkOption {
        internal = true;
        default = { };
        type = types.freeform;
      };

      # No-op option for now.
      users = lib.mkOption {
        internal = true;
        default = { };
        type = types.freeform;
      };

      networking = {
        enableIPv6 = lib.mkEnableOption "IPv6" // {
          default = true;
        };
      };

      system-manager = {
        allowAnyDistro = lib.mkEnableOption "the usage of system-manager on untested distributions";

        preActivationAssertions = lib.mkOption {
          type = with lib.types; attrsOf (submodule ({ name, ... }: {
            options = {
              enable = lib.mkEnableOption "the assertion";

              name = lib.mkOption {
                type = types.str;
                default = name;
              };

              script = lib.mkOption {
                type = types.str;
              };
            };
          }));
          default = { };
        };
      };
    };

  config = {
    system-manager.preActivationAssertions = {
      osVersion =
        let
          supportedIds = [ "nixos" "ubuntu" ];
        in
        {
          enable = !config.system-manager.allowAnyDistro;
          script = ''
            source /etc/os-release
            ${lib.concatStringsSep "\n" (lib.flip map supportedIds (supportedId: ''
              if [ $ID = "${supportedId}" ]; then
                exit 0
              fi
            ''))}
            echo "This OS is not currently supported."
            echo "Supported OSs are: ${lib.concatStringsSep ", " supportedIds}"
            exit 1
          '';
        };
    };

    # Can we make sure that this does not get relaunched when activating a new profile?
    # Otherwise we get an infinite loop.
    systemd.services.reactivate-system-manager = {
      enable = false;
      # TODO should we activate earlier?
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        Type = "oneshot";
      };
      script = ''
        /nix/var/nix/profiles/system-manager-profiles/system-manager/bin/activate
      '';
    };
  };
}

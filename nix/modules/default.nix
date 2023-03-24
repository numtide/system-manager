{ lib
, config
, ...
}:
{
  imports = [
    ./etc.nix
    ./systemd.nix
  ];

  options = {
    assertions = lib.mkOption {
      type = lib.types.listOf lib.types.unspecified;
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
      type = lib.types.listOf lib.types.str;
      example = [ "The `foo' service is deprecated and will go away soon!" ];
      description = lib.mdDoc ''
        This option allows modules to show warnings to users during
        the evaluation of the system configuration.
      '';
    };

    system-manager = {
      allowAnyDistro = lib.mkEnableOption "the usage of system-manager on untested distributions";

      preActivationAssertions = lib.mkOption {
        type = with lib.types; attrsOf (submodule ({ name, ... }: {
          options = {
            enable = lib.mkEnableOption "the assertion";

            name = lib.mkOption {
              type = lib.types.str;
              default = name;
            };

            script = lib.mkOption {
              type = lib.types.str;
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
  };
}

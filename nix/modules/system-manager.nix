{ lib
, config
, ...
}:
{
  imports = [
    ./etc.nix
    ./systemd.nix
  ];

  options.system-manager = {
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

  config = {
    # Avoid some standard NixOS assertions
    boot = {
      loader.grub.enable = false;
      initrd.enable = false;
    };
    system.stateVersion = lib.mkDefault lib.trivial.release;

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

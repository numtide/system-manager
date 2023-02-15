{ lib
, pkgs
, config
, ...
}:
{
  options.system-manager = {
    services = lib.mkOption {
      type = with lib.types; listOf str;
      default = [ ];
    };

    etcFiles = lib.mkOption {
      type = with lib.types; listOf str;
      default = [ ];
    };
  };

  config = {
    # Avoid some standard NixOS assertions
    boot = {
      loader.grub.enable = false;
      initrd.enable = false;
    };

    assertions = lib.flip map config.system-manager.etcFiles (entry:
      {
        assertion = lib.hasAttr entry config.environment.etc;
        message = lib.concatStringsSep " " [
          "The entry ${entry} that was passed to system-manager.etcFiles"
          "is not present in environment.etc"
        ];
      }
    );
  };
}

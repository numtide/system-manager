{ config, lib, ... }:
let
  inherit (lib) types;
in
{
  options = {
    systemd.tmpfiles.rules = lib.mkOption {
      type = types.listOf types.str;
      default = [ ];
      example = [ "d /tmp 1777 root root 10d" ];
      description = lib.mdDoc ''
        Rules for creation, deletion and cleaning of volatile and temporary files
        automatically. See
        {manpage}`tmpfiles.d(5)`
        for the exact format.
      '';
    };
  };

  config = {
    environment.etc."tmpfiles.d/00-system-manager.conf".text = ''
      # This file is created automatically and should not be modified.
      # Please change the option ‘systemd.tmpfiles.rules’ instead.
      ${lib.concatStringsSep "\n" config.systemd.tmpfiles.rules}
    '';
  };
}

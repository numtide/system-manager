{ lib, config, pkgs, ... }:

{
  options.environment = {
    systemPackages = lib.mkOption {
      type = lib.types.listOf lib.types.package;
      default = [ ];
    };
  };

  config = {
    environment.etc."profile.d/system-manager-path.sh".source =
      pkgs.writeShellScript "system-manager-path.sh" ''
        export PATH=${lib.makeBinPath config.environment.systemPackages}:''${PATH}
      '';
  };
}

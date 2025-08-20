{ lib, pkgs, ... }:
{
  options = {
    # options coming from modules/services/system/nix-daemon.nix that we cannot import just yet because it
    # depends on users. These are the minimum options we need to be able to configure Nix using system-manager.
    nix = {
      enable = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = ''
          Whether to enable Nix.
          Disabling Nix makes the system hard to modify and the Nix programs and configuration will not be made available by NixOS itself.
        '';
      };
      package = lib.mkOption {
        type = lib.types.package;
        default = pkgs.nix;
        defaultText = lib.literalExpression "pkgs.nix";
        description = ''
          This option specifies the Nix package instance to use throughout the system.
        '';
      };
    };
  };
}

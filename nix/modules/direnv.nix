{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.direnv;
in
{
  options.programs.direnv = {
    enable = lib.mkEnableOption "direnv integration";
    package = lib.mkPackageOption pkgs "direnv" { };
    nix-direnv = {
      enable = lib.mkEnableOption "nix-direnv integration";
      package = lib.mkPackageOption pkgs "nix-direnv" { };
    };
  };
  config = lib.mkIf cfg.enable {
    environment = {
      etc = {
        "profile.d/direnv.sh".source = pkgs.writeText "direnv.sh" ''
          eval "$(${lib.getExe cfg.package} hook bash)"
        '';
      };
      systemPackages =
        [
          cfg.package
        ]
        ++ lib.optionals cfg.nix-direnv.enable [
          cfg.nix-direnv.package
        ];
    };
  };
}

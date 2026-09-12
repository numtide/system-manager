{
  lib,
  config,
  ...
}:
let
  cfg = config.nix;
in
{
  options.nix.nixPath = lib.mkOption {
    type = lib.types.listOf lib.types.str;
    default = [ ];
    description = ''
      The default Nix expression search path, used by the Nix evaluator to
      look up paths enclosed in angle brackets (e.g. `<nixpkgs>`).
    '';
  };

  config = lib.mkMerge [
    {
      nix.enable = lib.mkDefault false;

      # Nix already owns its build users on the hosts we manage.
      # Priority 900 overrides the upstream mkDefault, users can still set it.
      nix.nrBuildUsers = lib.mkOverride 900 0;
    }

    (lib.mkIf config.nix.enable {
      environment.etc."nix/nix.conf".replaceExisting = true;
      environment.etc."nix/registry.json".replaceExisting = true;
      environment.sessionVariables = lib.mkIf (cfg.nixPath != [ ]) {
        NIX_PATH = cfg.nixPath;
      };
      nix.settings.experimental-features = lib.mkDefault [
        "nix-command"
        "flakes"
      ];
    })
  ];
}

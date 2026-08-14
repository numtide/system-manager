{
  lib,
  config,
  ...
}:
{
  config = lib.mkMerge [
    {
      nix.enable = lib.mkDefault false;

      # Nix already owns its build users on the hosts we manage.
      # Priority 900 overrides the upstream mkDefault, users can still set it.
      nix.nrBuildUsers = lib.mkOverride 900 0;
    }

    (lib.mkIf config.nix.enable {
      environment.etc."nix/nix.conf".replaceExisting = true;
      nix.settings.experimental-features = lib.mkDefault [
        "nix-command"
        "flakes"
      ];
    })
  ];
}

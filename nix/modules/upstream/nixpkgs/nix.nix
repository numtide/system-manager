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
      environment.systemPackages = [ config.nix.package ];

      # nix-flakes.nix always renders /etc/nix/registry.json, but we only want
      # to take over a pre-existing registry when we have something to put in
      # it. The original file is backed up and restored on deactivation.
      environment.etc."nix/registry.json" = {
        enable = config.nix.registry != { };
        replaceExisting = true;
      };

      nix.settings.experimental-features = lib.mkDefault [
        "nix-command"
        "flakes"
      ];
    })
  ];
}

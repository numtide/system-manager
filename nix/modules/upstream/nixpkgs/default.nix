{
  nixosModulesPath,
  lib,
  ...
}:
{
  imports =
    [
      ./nginx.nix
      ./nix.nix
      ./activation-script.nix
    ]
    ++
    # List of imported NixOS modules
    # TODO: how will we manage this in the long term?
    map (path: nixosModulesPath + path) [
      "/misc/meta.nix"
      "/security/acme/"
      "/services/web-servers/nginx/"
      # nix settings
      "/config/nix.nix"
    ];

  options =
    # We need to ignore a bunch of options that are used in NixOS modules but
    # that don't apply to system-manager configs.
    # TODO: can we print an informational message for things like kernel modules
    # to inform users that they need to be enabled in the host system?
    {
      boot = lib.mkOption {
        type = lib.types.raw;
      };
    };

}

{ nixosModulesPath
, ...
}:
{
  imports = [
    ./nginx.nix
  ] ++
  # List of imported NixOS modules
  # TODO: how will we manage this in the long term?
  map (path: nixosModulesPath + path) [
    "/misc/meta.nix"
    "/security/acme/"
    "/services/web-servers/nginx/"
  ];
}

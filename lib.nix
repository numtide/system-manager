{ nixpkgs }:
let
  inherit (nixpkgs) lib;
in
{
  makeServiceConfig = { system, module }:
    let
      nixosConfig = lib.nixosSystem {
        inherit system;
        specialArgs = { };
        modules = [ module ];
      };
      services = lib.flip lib.genAttrs
        (serviceName:
          nixosConfig.config.systemd.units."${serviceName}.service".unit)
        nixosConfig.config.service-manager.services;
    in
    nixpkgs.legacyPackages.${system}.writeTextFile {
      name = "services";
      destination = "/services.json";
      text = lib.generators.toJSON { } services;
    };
}

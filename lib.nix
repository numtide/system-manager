{ nixpkgs }:
let
  inherit (nixpkgs) lib;
in
{
  makeServiceConfig = { system, module }:
    let
      pkgs = nixpkgs.legacyPackages.${system};

      nixosConfig = lib.nixosSystem {
        inherit system;
        specialArgs = { };
        modules = [ module ];
      };
      services = lib.flip lib.genAttrs
        (serviceName:
          nixosConfig.config.systemd.units."${serviceName}.service".unit)
        nixosConfig.config.service-manager.services;

      servicesPath =
        pkgs.writeTextFile {
          name = "services";
          destination = "/services.json";
          text = lib.generators.toJSON { } services;
        };
      activationScript = pkgs.writeShellScript "activate" ''
        echo "${servicesPath}"
      '';
    in
    pkgs.linkFarmFromDrvs "service-manager" [
      servicesPath
      activationScript
    ];
}

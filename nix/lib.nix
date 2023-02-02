{ nixpkgs }:
let
  inherit (nixpkgs) lib;
in
{
  makeServiceConfig =
    { system
    , modules
    ,
    }:
    let
      pkgs = nixpkgs.legacyPackages.${system};

      nixosConfig = lib.nixosSystem {
        inherit system modules;
        specialArgs = { };
      };
      services =
        lib.flip lib.genAttrs
          (serviceName:
            nixosConfig.config.systemd.units."${serviceName}.service".unit)
          nixosConfig.config.service-manager.services;

      servicesPath = pkgs.writeTextFile {
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

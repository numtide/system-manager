{ nixpkgs }:
let
  inherit (nixpkgs) lib;
in
{
  makeServiceConfig =
    { system
    , modules
    , service-manager
    ,
    }:
    let
      pkgs = nixpkgs.legacyPackages.${system};

      nixosConfig = lib.nixosSystem {
        inherit system modules;
        specialArgs = { };
      };

      services = map
        (name: {
          inherit name;
          service = ''${nixosConfig.config.systemd.units."${name}.service".unit}/${name}.service'';
        })
        nixosConfig.config.service-manager.services;

      servicesPath = pkgs.writeTextFile {
        name = "services";
        destination = "/services.json";
        text = lib.generators.toJSON { } services;
      };
      activationScript = pkgs.writeShellScript "activate" ''
        ${service-manager}/bin/service-manager activate \
          --store-path "$(realpath $(dirname ''${0}))"
      '';
    in
    pkgs.linkFarmFromDrvs "service-manager" [
      servicesPath
      activationScript
    ];
}

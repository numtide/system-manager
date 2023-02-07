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

      services =
        map
          (name:
            let
              serviceName = "${name}.service";
            in
            {
              name = serviceName;
              service = ''${nixosConfig.config.systemd.units."${serviceName}".unit}/${serviceName}'';
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

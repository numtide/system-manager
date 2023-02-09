{ nixpkgs }:
let
  inherit (nixpkgs) lib;
in
{
  makeServiceConfig =
    { system
    , modules
    , system-manager
    ,
    }:
    let
      pkgs = nixpkgs.legacyPackages.${system};

      nixosConfig = lib.nixosSystem {
        inherit system modules;
        specialArgs = { };
      };

      services =
        lib.listToAttrs
          (map
            (name:
              let
                serviceName = "${name}.service";
              in
              lib.nameValuePair serviceName { storePath = ''${nixosConfig.config.systemd.units."${serviceName}".unit}/${serviceName}''; })
            nixosConfig.config.system-manager.services);

      servicesPath = pkgs.writeTextFile {
        name = "services";
        destination = "/services.json";
        text = lib.generators.toJSON { } services;
      };
      activationScript = pkgs.writeShellScript "activate" ''
        ${system-manager}/bin/system-manager activate \
          --store-path "$(realpath $(dirname ''${0}))"
      '';
    in
    pkgs.linkFarmFromDrvs "system-manager" [
      servicesPath
      activationScript
    ];
}

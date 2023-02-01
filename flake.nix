{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }: {
    serviceConfig = self.lib.makeServiceConfig
      [
        "service-1"
        "service-2"
      ];

    lib = {
      makeServiceConfig = serviceNames:
        let
          system = flake-utils.lib.system.x86_64-linux;
          lib = nixpkgs.lib;
          nixosConfig = nixpkgs.lib.nixosSystem {
            inherit system;
            specialArgs = { };
            modules = [ ./modules ];
          };
          services = lib.flip lib.genAttrs
            (serviceName:
              nixosConfig.config.systemd.units."${serviceName}.service".unit)
            serviceNames;
        in
        nixpkgs.legacyPackages.${system}.writeTextFile {
          name = "services";
          destination = "/services.json";
          text = lib.generators.toJSON { } services;
        };
    };
  };
}

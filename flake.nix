{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }: {
    serviceConfig = self.lib.makeServiceConfig {
      system = flake-utils.lib.system.x86_64-linux;
      module = { imports = [ ./modules ]; };
    };

    lib = {
      makeServiceConfig = { system, module }:
        let
          lib = nixpkgs.lib;
          nixosConfig = nixpkgs.lib.nixosSystem {
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
    };
  };
}

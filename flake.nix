{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }: {
    serviceConfig =
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
          # TODO: generate this list from the config instead of hard coding
          [
            "service-1"
            "service-2"
          ];
      in
      nixpkgs.legacyPackages.${system}.writeTextFile {
        name = "services";
        destination = "/services.json";
        text = lib.generators.toJSON { } services;
      };
  };
}

{
  description = "Manage system configurations using Nix on any Linux distribution.";

  nixConfig = {
    extra-substituters = [ "https://cache.numtide.com" ];
    extra-trusted-public-keys = [ "niks3.numtide.com-1:DTx8wZduET09hRmMtKdQDxNNthLQETkc/yaX7M4qK0g=" ];
  };

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.userborn = {
    url = "github:jfroche/userborn/system-manager";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    {
      self,
      nixpkgs,
      userborn,
    }:
    let
      systems = [
        "aarch64-linux"
        "aarch64-darwin"
        "x86_64-linux"
      ];
      eachSystem =
        f:
        nixpkgs.lib.genAttrs systems (
          system:
          f {
            inherit system;
            pkgs = nixpkgs.legacyPackages.${system};
          }
        );
    in
    {
      lib = (import ./nix/lib.nix { inherit nixpkgs userborn; }) // {
        # Container test library for external projects
        containerTest = import ./lib/container-test-driver { inherit (nixpkgs) lib; };
      };

      packages = eachSystem (
        { pkgs, system }:
        rec {
          system-manager-unwrapped = pkgs.callPackage ./package.nix { };
          default = pkgs.callPackage ./nix/packages/wrapper.nix { inherit system-manager-unwrapped; };
        }
      );

      # Documentation outputs
      docs = eachSystem ({ pkgs, ... }: import ./docs/options.nix { inherit pkgs; });

      overlays = {
        default = final: _prev: rec {
          system-manager-unwrapped = final.callPackage ./package.nix { };
          system-manager = final.callPackage ./nix/packages/wrapper.nix { inherit system-manager-unwrapped; };
        };
      };

      # Only useful for quick tests
      systemConfigs.default = self.lib.makeSystemConfig {
        modules = [
          ./examples/example.nix
          { nixpkgs.hostPlatform = "x86_64-linux"; }
        ];
      };

      formatter = eachSystem ({ pkgs, ... }: pkgs.treefmt);

      devShells = eachSystem (
        { pkgs, ... }:
        {
          default = import ./shell.nix { inherit pkgs; };
        }
      );

      checks = eachSystem (
        { system, ... }:
        {
          system-manager = self.packages.${system}.default;
        }
      );

      nixosModules = rec {
        system-manager = ./nix/modules;
        default = system-manager;
      };

      templates = {
        standalone = {
          path = ./templates/standalone;
          description = "System Manager standalone setup";
        };
        nixos = {
          path = ./templates/nixos;
          description = "System Manager as a NixOS module";
        };
      };
    };
}

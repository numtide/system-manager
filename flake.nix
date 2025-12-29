{
  description = "Manage system configurations using Nix on any Linux distribution.";

  nixConfig = {
    extra-substituters = [ "https://cache.numtide.com" ];
    extra-trusted-public-keys = [ "niks3.numtide.com-1:DTx8wZduET09hRmMtKdQDxNNthLQETkc/yaX7M4qK0g=" ];
  };

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "aarch64-linux"
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
      nix-vm-test-lib =
        let
          rev = "991369a72fe577c2bcdad0b26bf8c63a6f94f84b";
          sha256 = "sha256:1ygn0acvzzrg0jbnbpwfl4n4k2ka6ay0x34sj61g11c1pvckl3m9";
        in
        "${
          builtins.fetchTarball {
            url = "https://github.com/numtide/nix-vm-test/archive/${rev}.tar.gz";
            inherit sha256;
          }
        }/lib.nix";
    in
    {
      lib = import ./nix/lib.nix { inherit nixpkgs; };

      packages = eachSystem (
        { pkgs, system }:
        {
          default = pkgs.callPackage ./package.nix { };
        }
      );

      overlays = {
        default = final: _prev: {
          system-manager = final.callPackage ./package.nix { };
        };
      };

      # Only useful for quick tests
      systemConfigs.default = self.lib.makeSystemConfig {
        modules = [ ./examples/example.nix ];
      };

      formatter = eachSystem ({ pkgs, ... }: pkgs.treefmt);

      devShells = eachSystem (
        { pkgs, ... }:
        {
          default = import ./shell.nix { inherit pkgs; };
        }
      );

      checks = (
        nixpkgs.lib.recursiveUpdate
          (eachSystem (
            { system, ... }:
            {
              system-manager = self.packages.${system}.default;
            }
          ))
          {
            x86_64-linux =
              let
                system = "x86_64-linux";
              in
              (import ./test/nix/modules {
                inherit system;
                inherit (nixpkgs) lib;
                nix-vm-test = import nix-vm-test-lib {
                  inherit nixpkgs;
                  inherit system;
                };
                system-manager = self;
              });
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

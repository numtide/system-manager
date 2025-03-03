{
  description = "Manage system configurations using Nix on any Linux distribution.";

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
          rev = "f01e096295c8ace5f367985e323e3263eb7f9434";
          sha256 = "sha256:11gqxnhdrpb025wybbb0wmpy2xzjaa6ncs55zbw8i2nzchkzrfvh";
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
        import ./packages.nix { inherit pkgs; }
        // {
          default = self.packages.${system}.system-manager;
        }
      );

      overlays = {
        packages = final: _prev: import ./packages.nix { pkgs = final; };
        default = self.overlays.packages;
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
              system-manager = self.packages.${system}.system-manager;
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

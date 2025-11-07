{
  description = "Manage system config using nix on any distro";

  nixConfig = {
    extra-substituters = [ "https://numtide.cachix.org" ];
    extra-trusted-public-keys = [ "numtide.cachix.org-1:2ps1kLBUWjxIneOy1Ik6cQjb41X0iXVXeHigGmycPPE=" ];
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
          rev = "e34870b8dd2c2d203c05b4f931b8c33eaaf43b81";
          sha256 = "sha256:1qp1fq96kv9i1nj20m25057pfcs1b1c9bj4502xy7gnw8caqr30d";
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
    };
}

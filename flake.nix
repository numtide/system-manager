{
  description = "Manage system config using nix on any distro";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    {
      self,
      nixpkgs,
    }:
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
    };
}

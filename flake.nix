{
  description = "Manage system config using nix on any distro";

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
                nix-vm-test-src = builtins.fetchTarball {
                  url = "https://github.com/numtide/nix-vm-test/archive/7901cec00670681b3e405565cb7bffe6a9368240.tar.gz";
                  sha256 = "0m82a40r3j7qinp3y6mh36da89dkwvpalz6a4znx9rqp6kh3885x";
                };
                nix-vm-test = import "${nix-vm-test-src}/lib.nix" {
                  inherit nixpkgs;
                  inherit system;
                };
              in
              (import ./test/nix/modules {
                inherit system;
                inherit (nixpkgs) lib;
                inherit nix-vm-test;
                system-manager = self;
              });
          }
      );
    };
}

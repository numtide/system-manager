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
      nix-vm-test-lib = "${
        builtins.fetchTarball {
          url = "https://github.com/numtide/nix-vm-test/archive/21816a2e64f35a1f5b27dadd067e051606c2b451.tar.gz";
          sha256 = "1mbs97bvi6g892b0s9dwq0yj3hszrxf0yivw9v89llvl8qxz2qi8";
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
    };
}

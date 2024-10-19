{
  description = "Manage system config using nix on any distro";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    nix-vm-test = {
      url = "github:numtide/nix-vm-test";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs:
    let
      systems = [
        "aarch64-linux"
        "x86_64-linux"
      ];
      eachSystem =
        f:
        inputs.nixpkgs.lib.genAttrs systems (
          system:
          f {
            inherit system;
            pkgs = inputs.nixpkgs.legacyPackages.${system};
          }
        );
    in
    {
      lib = import ./nix/lib.nix { inherit (inputs) nixpkgs; };

      packages = eachSystem (
        { pkgs, system }:
        import ./packages.nix { inherit pkgs; }
        // {
          default = inputs.self.packages.${system}.system-manager;
        }
      );

      overlays = {
        packages = final: _prev: import ./packages.nix { pkgs = final; };
        default = inputs.self.overlays.packages;
      };

      # Only useful for quick tests
      systemConfigs.default = inputs.self.lib.makeSystemConfig {
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
        inputs.nixpkgs.lib.recursiveUpdate
          (eachSystem (
            { system, ... }:
            {
              system-manager = inputs.self.packages.${system}.system-manager;
            }
          ))
          {
            x86_64-linux =
              let
                system = "x86_64-linux";
              in
              (import ./test/nix/modules {
                inherit system;
                inherit (inputs.nixpkgs) lib;
                nix-vm-test = inputs.nix-vm-test.lib.${system};
                system-manager = inputs.self;
              });
          }
      );
    };
}

{
  description = "Manage system configurations using Nix on any Linux distribution.";

  nixConfig = {
    extra-substituters = [ "https://cache.numtide.com" ];
    extra-trusted-public-keys = [ "niks3.numtide.com-1:DTx8wZduET09hRmMtKdQDxNNthLQETkc/yaX7M4qK0g=" ];
  };

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.flake-compat = {
    url = "github:edolstra/flake-compat";
    flake = false;
  };
  inputs.userborn = {
    url = "github:jfroche/userborn/system-manager";
    inputs.nixpkgs.follows = "nixpkgs";
    inputs.flake-compat.follows = "flake-compat";
  };

  outputs =
    {
      self,
      nixpkgs,
      userborn,
      ...
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
      packageOverlay = final: _prev: rec {
        system-manager-unwrapped = final.callPackage ./package.nix { };
        system-manager = final.callPackage ./nix/packages/wrapper.nix { inherit system-manager-unwrapped; };
      };
    in
    {
      lib = (import ./nix/lib.nix { inherit nixpkgs userborn; }) // {
        # Container test library for external projects
        containerTest = import ./lib/container-test-driver { inherit (nixpkgs) lib; };
      };

      # Get overlayed packages in toplevel output
      packages = eachSystem (
        { pkgs, system }:
        {
          system-manager-unwrapped = pkgs.system-manager-unwrapped;
          default = pkgs.system-manager;
        }
      );

      # Documentation outputs
      docs = eachSystem ({ pkgs, ... }: import ./docs/options.nix { inherit pkgs; });

      overlays.default = packageOverlay;
      # Overlay packages onto internal nixpkgs
      nixpkgs.overlays = [ packageOverlay ];

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
          default =
            let
              llvm = pkgs.llvmPackages_latest;
            in
            pkgs.mkShellNoCC {
              shellHook = ''
                ${pkgs.pre-commit}/bin/pre-commit install --install-hooks --overwrite
                export PKG_CONFIG_PATH="${pkgs.lib.makeSearchPath "lib/pkgconfig" [ pkgs.dbus.dev ]}"
                export LIBCLANG_PATH="${llvm.libclang}/lib"
                # for rust-analyzer
                export RUST_SRC_PATH="${pkgs.rustPlatform.rustLibSrc}"
                export RUST_BACKTRACE=1
              '';
              buildInputs = [
                pkgs.dbus
              ]
              ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [ pkgs.libiconv ];
              nativeBuildInputs = with pkgs; [
                llvm.clang
                pkg-config
                rustc
                cargo
                # Formatting
                pre-commit
                treefmt
                nixfmt
                rustfmt
                clippy
                mdbook
                mdformat
                rust-analyzer
                gh
                # Testing tools
                parallel
              ];
            };
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

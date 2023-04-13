{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    devshell = {
      url = "github:numtide/devshell";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
    pre-commit-hooks = {
      url = "github:cachix/pre-commit-hooks.nix";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
        rust-overlay.follows = "rust-overlay";
        flake-compat.follows = "pre-commit-hooks/flake-compat";
      };
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { self
    , nixpkgs
    , flake-utils
    , rust-overlay
    , crane
    , devshell
    , treefmt-nix
    , pre-commit-hooks
    ,
    }:
    {
      lib = import ./nix/lib.nix {
        inherit nixpkgs self;
        nixos = "${nixpkgs}/nixos";
      };
    }
    //
    (flake-utils.lib.eachSystem
      [
        flake-utils.lib.system.x86_64-linux
        flake-utils.lib.system.aarch64-linux
      ]
      (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) devshell.overlays.default ];
        };
        # TODO Pin the version for release
        rust = pkgs.rust-bin.stable.latest;
        llvm = pkgs.llvmPackages_latest;

        craneLib = (crane.mkLib pkgs).overrideToolchain rust.default;

        # Common derivation arguments used for all builds
        commonArgs = {
          src = craneLib.cleanCargoSource ./.;
          buildInputs = with pkgs; [
            dbus
          ];
          nativeBuildInputs = with pkgs; [
            pkg-config
          ];
        };

        # Build only the cargo dependencies
        cargoArtifacts = craneLib.buildDepsOnly (commonArgs // {
          pname = "system-manager";
        });

        system-manager = craneLib.buildPackage (commonArgs // {
          pname = "system-manager";
          inherit cargoArtifacts;
        });

        system-manager-clippy = craneLib.cargoClippy (commonArgs // {
          inherit cargoArtifacts;
          cargoClippyExtraArgs = "--all-targets -- --deny warnings";
        });

        system-manager-test = craneLib.cargoTest (commonArgs // {
          inherit cargoArtifacts;
        });

        # treefmt-nix configuration
        treefmt.config = {
          projectRootFile = "flake.nix";
          programs = {
            nixpkgs-fmt.enable = true;
            rustfmt = {
              enable = true;
              package = rust.rustfmt;
            };
          };
        };
      in
      {
        packages = {
          inherit system-manager;
          default = self.packages.${system}.system-manager;
        };

        devShells.default = pkgs.devshell.mkShell {
          packages = with pkgs; [
            llvm.clang
            openssl
            pkg-config
            (rust.default.override {
              extensions = [ "rust-src" ];
            })
            (treefmt-nix.lib.mkWrapper pkgs treefmt.config)
          ];
          env = [
            {
              name = "PKG_CONFIG_PATH";
              value = pkgs.lib.makeSearchPath "lib/pkgconfig" [
                pkgs.dbus.dev
                pkgs.systemdMinimal.dev
              ];
            }
            {
              name = "LIBCLANG_PATH";
              value = "${llvm.libclang}/lib";
            }
            {
              # for rust-analyzer
              name = "RUST_SRC_PATH";
              value = "${rust.rust-src}";
            }
            {
              name = "RUST_BACKTRACE";
              value = "1";
            }
            {
              name = "RUSTFLAGS";
              value =
                let
                  getLib = pkg: "${pkgs.lib.getLib pkg}/lib";
                in
                pkgs.lib.concatStringsSep " " [
                  "-L${getLib pkgs.systemdMinimal} -lsystemd"
                ];
            }
            {
              name = "DEVSHELL_NO_MOTD";
              value = "1";
            }
          ];
          devshell.startup.pre-commit.text = (pre-commit-hooks.lib.${system}.run {
            src = ./.;
            hooks = {
              check-format = {
                enable = true;
                entry = "treefmt --fail-on-change";
              };
              cargo-clippy = {
                enable = true;
                description = "Lint Rust code.";
                entry = "cargo-clippy --workspace -- -D warnings";
                files = "\\.rs$";
                pass_filenames = false;
              };
            };
          }).shellHook;
        };

        checks = {
          inherit
            # Build the crate as part of `nix flake check` for convenience
            system-manager
            system-manager-clippy
            system-manager-test;
        };
      }));
}

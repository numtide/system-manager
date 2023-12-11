{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    devshell = {
      url = "github:numtide/devshell";
      inputs = {
        nixpkgs.follows = "nixpkgs";
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

      # Only useful for quick tests
      systemConfigs.default = self.lib.makeSystemConfig {
        modules = [ ./examples/example.nix ];
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

        craneLib = (crane.mkLib pkgs).overrideToolchain rust.default;

        # Common derivation arguments used for all builds
        commonArgs = { dbus, pkg-config }: {
          src = craneLib.cleanCargoSource ./.;
          buildInputs = [
            dbus
          ];
          nativeBuildInputs = [
            pkg-config
          ];
          # https://github.com/ipetkov/crane/issues/385
          doNotLinkInheritedArtifacts = true;
        };

        # Build only the cargo dependencies
        cargoArtifacts = { dbus, pkg-config }:
          craneLib.buildDepsOnly ((commonArgs { inherit dbus pkg-config; }) // {
            pname = "system-manager";
          });

        system-manager-unwrapped =
          { dbus
          , pkg-config
          }:
          craneLib.buildPackage ((commonArgs { inherit dbus pkg-config; }) // {
            pname = "system-manager";
            cargoArtifacts = cargoArtifacts { inherit dbus pkg-config; };
          });

        system-manager =
          { dbus
          , makeBinaryWrapper
          , nix
          , pkg-config
          , runCommand
          }:
          let
            unwrapped = system-manager-unwrapped { inherit dbus pkg-config; };
          in
          runCommand "system-manager"
            {
              nativeBuildInputs = [ makeBinaryWrapper ];
            }
            ''
              makeWrapper \
                ${unwrapped}/bin/system-manager \
                $out/bin/system-manager \
                --prefix PATH : ${nixpkgs.lib.makeBinPath [ nix ]}
            '';

        system-manager-clippy =
          { dbus
          , pkg-config
          }:
          craneLib.cargoClippy ((commonArgs { inherit dbus pkg-config; }) // {
            cargoArtifacts = cargoArtifacts { inherit dbus pkg-config; };
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });

        system-manager-test =
          { dbus
          , pkg-config
          }:
          craneLib.cargoTest ((commonArgs { inherit dbus pkg-config; }) // {
            cargoArtifacts = cargoArtifacts { inherit dbus pkg-config; };
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
          # The unwrapped version takes nix from the PATH, it will fail if nix
          # cannot be found.
          # The wrapped version has a reference to the nix store path, so nix is
          # part of its runtime closure.
          system-manager-unwrapped = pkgs.callPackage system-manager-unwrapped { };
          system-manager = pkgs.callPackage system-manager { };

          system-manager-clippy = pkgs.callPackage system-manager-clippy { };
          system-manager-test = pkgs.callPackage system-manager-test { };

          default = self.packages.${system}.system-manager;
        };

        devShells.default =
          let
            llvm = pkgs.llvmPackages_latest;
          in
          pkgs.devshell.mkShell {
            packages = with pkgs; [
              llvm.clang
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

        checks =
          let
            # The Aarch64 VM tests seem to hang on garnix, we disable them for now
            enableVmTests = system != flake-utils.lib.system.aarch64-linux;
          in
          {
            inherit (self.packages.${system})
              # Build the crate as part of `nix flake check` for convenience
              system-manager
              system-manager-clippy
              system-manager-test;
          } //
          pkgs.lib.optionalAttrs enableVmTests (import ./test/nix/modules {
            inherit system;
            inherit (pkgs) lib;
            system-manager = self;
          });
      })
    );
}

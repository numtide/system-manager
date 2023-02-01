{
  inputs = {
    devshell.url = "github:numtide/devshell";
    flake-utils.url = "github:numtide/flake-utils";
    nix-filter.url = "github:numtide/nix-filter";
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    pre-commit-hooks.url = "github:cachix/pre-commit-hooks.nix";
    rust-overlay.url = "github:oxalica/rust-overlay";
    treefmt-nix.url = "github:numtide/treefmt-nix";
  };

  outputs =
    { self
    , nixpkgs
    , flake-utils
    , nix-filter
    , rust-overlay
    , devshell
    , treefmt-nix
    , pre-commit-hooks
    ,
    }:
    flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ (import rust-overlay) devshell.overlay ];
      };
      rust = pkgs.rust-bin.stable."1.64.0";
      llvm = pkgs.llvmPackages_latest;
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
    rec {
      serviceConfig = lib.makeServiceConfig {
        inherit system;
        module = { imports = [ ./nix/modules ]; };
      };

      lib = import ./nix/lib.nix { inherit nixpkgs; };

      packages = rec {
        service-manager =
          pkgs.rustPlatform.buildRustPackage
            {
              pname = "service-manager";
              version = (pkgs.lib.importTOML ./Cargo.toml).package.version;

              src = nix-filter.lib {
                root = ./.;
                include = [ "Cargo.toml" "Cargo.lock" (nix-filter.lib.inDirectory "src") ];
              };

              cargoLock.lockFile = ./Cargo.lock;
            };
        default = service-manager;
      };
      devShells.default = pkgs.devshell.mkShell {
        packages = with pkgs; [
          llvm.clang
          openssl
          pkg-config
          rust.default
          (treefmt-nix.lib.mkWrapper pkgs treefmt.config)
        ];
        env = [
          {
            name = "LIBCLANG_PATH";
            value = "${llvm.libclang}/lib";
          }
          {
            # for rust-anaylzer
            name = "RUST_SRC_PATH";
            value = "${rust.rust-src}";
          }
          {
            name = "RUST_BACKTRACE";
            value = "1";
          }
          {
            name = "RUST_LOG";
            value = "info";
          }
          {
            name = "DEVSHELL_NO_MOTD";
            value = "1";
          }
        ];
        devshell.startup.pre-commit.text = self.checks.${system}.pre-commit-check.shellHook;
      };
      checks = {
        pre-commit-check = pre-commit-hooks.lib.${system}.run {
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
        };
      };
    });
}

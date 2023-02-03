{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    devshell = {
      url = "github:numtide/devshell";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };
    nix-filter.url = "github:numtide/nix-filter";
    pre-commit-hooks = {
      url = "github:cachix/pre-commit-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };
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
    (flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ (import rust-overlay) devshell.overlay ];
      };
      rust = pkgs.rust-bin.stable."1.66.0";
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
    {
      serviceConfig = self.lib.makeServiceConfig {
        inherit system;
        modules = [
          ./nix/modules
        ];
        inherit (self.packages.${system}) service-manager;
      };

      packages = {
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

              nativeBuildInputs = with pkgs; [
                pkg-config
              ];
              buildInputs = with pkgs; [
                dbus
              ];
            };
        default = self.packages.${system}.service-manager;
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
            value = "${pkgs.lib.getOutput "dev" pkgs.dbus}/lib/pkgconfig";
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
    }))
    //
    {
      lib = import ./nix/lib.nix { inherit nixpkgs; };
    };
}

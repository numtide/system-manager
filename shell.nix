{
  pkgs ? import <nixpkgs> { },
}:
let
  llvm = pkgs.llvmPackages_latest;
in
pkgs.mkShellNoCC {
  shellHook = ''
    ${pkgs.pre-commit}/bin/pre-commit install --install-hooks --overwrite
    export PKG_CONFIG_PATH="${
      pkgs.lib.makeSearchPath "lib/pkgconfig" [
        pkgs.dbus.dev
        pkgs.systemdMinimal.dev
      ]
    }"
    export LIBCLANG_PATH="${llvm.libclang}/lib"
    # for rust-analyzer
    export RUST_SRC_PATH="${pkgs.rustPlatform.rustLibSrc}"
    export RUST_BACKTRACE=1
    export RUSTFLAGS="${
      pkgs.lib.concatStringsSep " " [
        "-L${pkgs.lib.getLib pkgs.systemdMinimal}/lib"
        "-lsystemd"
      ]
    }"
  '';
  buildInputs = with pkgs; [
    dbus
  ];
  nativeBuildInputs = with pkgs; [
    llvm.clang
    pkg-config
    rustc
    cargo
    # Formatting
    pre-commit
    treefmt
    nixfmt-rfc-style
    rustfmt
    clippy
    mdbook
    mdformat
    rust-analyzer
    gh
  ];
}

{
  lib,

  rustPlatform,
  cargo,
  dbus,
  pkg-config,
  nix,
  clippy,
}:

let
  cargoManifest = lib.importTOML ./Cargo.toml;
in
rustPlatform.buildRustPackage {
  pname = "system-manager";
  version = cargoManifest.workspace.package.version;
  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [
      ./Cargo.toml
      ./Cargo.lock
      ./crates
      ./templates
    ];
  };

  cargoLock.lockFile = ./Cargo.lock;
  buildInputs = [ dbus ];
  nativeBuildInputs = [
    pkg-config
  ];

  nativeCheckInputs = [
    clippy
    nix
    cargo
  ];

  preCheck = ''
    cargo clippy

    # Stop the Nix command from trying to create /nix/var/nix/profiles.
    #
    # https://nix.dev/manual/nix/2.24/command-ref/new-cli/nix3-profile#profiles
    export NIX_STATE_DIR=$TMPDIR
  '';
}

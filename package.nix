{
  lib,

  rustPlatform,
  cargo,
  dbus,
  pkg-config,
  nix,
  clippy,
  writableTmpDirAsHomeHook,
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
    writableTmpDirAsHomeHook
  ];

  preCheck = ''
    cargo clippy

    # Stop the Nix command from trying to create /nix/var/nix/profiles.
    #
    # https://nix.dev/manual/nix/2.24/command-ref/new-cli/nix3-profile#profiles
    export NIX_STATE_DIR=$TMPDIR
  '';

  meta = {
    description = "Manage system configurations using Nix on any Linux distribution";
    homepage = "https://github.com/numtide/system-manager";
    license = lib.licenses.mit;
    platforms = [
      "aarch64-linux"
      "aarch64-darwin"
      "x86_64-linux"
    ];
    mainProgram = "system-manager";
  };
}

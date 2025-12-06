{
  lib,

  rustPlatform,
  cargo,
  dbus,
  pkg-config,
  nix,
  clippy,

  runCommand,
  makeBinaryWrapper,
}:
let
  cargoManifest = (lib.importTOML ./Cargo.toml).package;
  system-manager-unwrapped = rustPlatform.buildRustPackage {
    pname = "system-manager";
    version = cargoManifest.version;
    src = lib.fileset.toSource {
      root = ./.;
      fileset = lib.fileset.unions [
        ./Cargo.toml
        ./Cargo.lock
        ./src
        ./test/rust
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
  };
in
runCommand "system-manager"
  {
    nativeBuildInputs = [ makeBinaryWrapper ];
    # The unwrapped version takes nix from the PATH, it will fail if nix
    # cannot be found.
    # The wrapped version has a reference to the nix store path, so nix is
    # part of its runtime closure.
    unwrapped = system-manager-unwrapped;
  }
  ''
    # Wrap the CLI binary with nix in PATH
    makeWrapper \
      $unwrapped/bin/system-manager \
      $out/bin/system-manager \
      --prefix PATH : ${lib.makeBinPath [ nix ]}

    # Wrap the engine binary with nix in PATH (needed for register command)
    makeWrapper \
      ${system-manager-unwrapped}/bin/system-manager-engine \
      $out/bin/system-manager-engine \
      --prefix PATH : ${lib.makeBinPath [ nix ]}
  ''

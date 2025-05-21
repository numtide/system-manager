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
      ];

      preCheck = ''
        ${lib.getExe cargo} clippy

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
        passthru = {
          # The unwrapped version takes nix from the PATH, it will fail if nix
          # cannot be found.
          # The wrapped version has a reference to the nix store path, so nix is
          # part of its runtime closure.
          unwrapped = system-manager-unwrapped;
        };
      }
      ''
        makeWrapper \
          ${system-manager-unwrapped}/bin/system-manager \
          $out/bin/system-manager \
          --prefix PATH : ${lib.makeBinPath [ nix ]}
      ''

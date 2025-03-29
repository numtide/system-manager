{
  pkgs ? import <nixpkgs> { },
  lib ? pkgs.lib,
}:
let
  cargoManifest = (pkgs.lib.importTOML ./Cargo.toml).package;
  system-manager-unwrapped = pkgs.callPackage (
    {
      rustPlatform,
      dbus,
      pkg-config,
      nix,
      clippy,
    }:
    rustPlatform.buildRustPackage {
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
        ${lib.getExe pkgs.cargo} clippy

        # Stop the Nix command from trying to create /nix/var/nix/profiles.
        #
        # https://nix.dev/manual/nix/2.24/command-ref/new-cli/nix3-profile#profiles
        export NIX_STATE_DIR=$TMPDIR
      '';
    }
  ) { };
in
{
  inherit system-manager-unwrapped;
  system-manager =
    pkgs.runCommand "system-manager"
      {
        nativeBuildInputs = [ pkgs.makeBinaryWrapper ];
      }
      ''
        makeWrapper \
          ${system-manager-unwrapped}/bin/system-manager \
          $out/bin/system-manager \
          --prefix PATH : ${lib.makeBinPath [ pkgs.nix ]}
      '';
}

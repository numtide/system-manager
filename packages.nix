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
        ];
      };

      cargoLock.lockFile = ./Cargo.lock;
      buildInputs = [ dbus ];
      nativeBuildInputs = [
        pkg-config
      ];

      nativeCheckInputs = [
        clippy
      ];

      preCheck = ''
        ${lib.getExe pkgs.cargo} clippy
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

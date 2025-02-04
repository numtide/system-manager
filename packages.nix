{
  pkgs ? import <nixpkgs> { },
  lib ? pkgs.lib,
}:
let
  cargoManifest = (pkgs.lib.importTOML ./Cargo.toml).package;
in
{
  system-manager = pkgs.callPackage (
    {
      rustPlatform,
      dbus,
      pkg-config,
      makeWrapper,
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
        makeWrapper
      ];

      nativeCheckInputs = [
        clippy
      ];

      preCheck = ''
        ${lib.getExe pkgs.cargo} clippy
      '';

      # TODO: Is prefixing nix here the correct approach?
      postFixup = ''
        wrapProgram $out/bin/system-manager \
          --prefix PATH : ${lib.makeBinPath [ nix ]}
      '';
    }
  ) { };
}

{
  pkgs ? import <nixpkgs> { },
  lib ? pkgs.lib,
}:
let
  # This project's `.gitignore` implemented for cleanSource.
  filterGitignore =
    orig_path: type:
    let
      baseName = baseNameOf (toString orig_path);
    in
    !(baseName == "target" && type == "directory")
    || lib.hasSuffix ".rs.bk" baseName
    || baseName == ".nixos-test-history"
    || (baseName == ".direnv" && type == "directory");

  cleanSourceWithGitignore =
    src:
    lib.cleanSourceWith {
      src = lib.cleanSource src;
      filter = filterGitignore;
    };

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
    }:
    rustPlatform.buildRustPackage {
      pname = "system-manager";
      version = cargoManifest.version;
      src = cleanSourceWithGitignore ./.;
      cargoLock.lockFile = ./Cargo.lock;
      buildInputs = [ dbus ];
      nativeBuildInputs = [
        pkg-config
        makeWrapper
      ];
      checkType = "debug"; # might not be required?
      # TODO: Is prefixing nix here the correct approach?
      postFixup = ''
        wrapProgram $out/bin/system-manager \
          --prefix PATH : ${lib.makeBinPath [ nix ]}
      '';
    }
  ) { };
}

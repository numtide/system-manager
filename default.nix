{
  nixpkgs ? <nixpkgs>,
  pkgs ? import nixpkgs { },
}:
{
  lib = import ./nix/lib.nix { inherit nixpkgs; };
  system-manager = pkgs.callPackage ./package.nix { };
}

{
  nixpkgs ? <nixpkgs>,
  pkgs ? import nixpkgs { },
}:
rec {
  lib = import ./nix/lib.nix { inherit nixpkgs; };
  system-manager-unwrapped = pkgs.callPackage ./package.nix { };
  system-manager = pkgs.callPackage ./nix/packages/wrapper.nix { inherit system-manager-unwrapped; };
}

{
  nixpkgs ? <nixpkgs>,
  pkgs ? import nixpkgs { },
}:
{
  lib = import ./nix/lib.nix { inherit nixpkgs; };
}
// import ./packages.nix { inherit pkgs; }

# Container tests using systemd-nspawn.
{
  lib,
  system-manager,
  system,
  hostPkgs,
  nixpkgs,
  sops-nix,
  system-manager-v1-1-0,
  home-manager,
}:

let
  containerTestLib = import ../../lib/container-test-driver { inherit lib; };
  distros = import ../../lib/container-test-driver/distros.nix { pkgs = hostPkgs; };
  supportedDistros = lib.filterAttrs (_: d: builtins.elem system d.systems) distros;

  makeContainerTestFor =
    distroName: distroConfig: name:
    {
      modules,
      testScriptFunction,
      extraPathsToRegister ? [ ],
    }:
    let
      toplevel = system-manager.lib.makeSystemConfig {
        modules = modules ++ [
          (
            { lib, pkgs, ... }:
            {
              options.hostPkgs = lib.mkOption {
                type = lib.types.raw;
                readOnly = true;
              };
              config = {
                nixpkgs.hostPlatform = system;
                hostPkgs = pkgs;
                system-manager.allowAnyDistro = true;
              };
            }
          )
        ];
      };
    in
    containerTestLib.makeContainerTest {
      inherit hostPkgs toplevel;
      inherit (distroConfig) rootfs;
      name = builtins.replaceStrings [ "_" ] [ "-" ] "${distroName}-${name}";
      testScript = testScriptFunction { inherit toplevel hostPkgs distroConfig; };
      extraPathsToRegister = extraPathsToRegister ++ [ toplevel ];
    };

  forEachDistro =
    name: testConfig:
    lib.mapAttrs' (
      distroName: distroConfig:
      lib.nameValuePair "container-${distroName}-${name}" (
        makeContainerTestFor distroName distroConfig name testConfig
      )
    ) supportedDistros;

  callTest =
    file:
    import file {
      inherit
        forEachDistro
        makeContainerTestFor
        system-manager
        system
        nixpkgs
        lib
        hostPkgs
        sops-nix
        system-manager-v1-1-0
        home-manager
        ;
    };
  testFiles = lib.filterAttrs (name: type: name != "default.nix" && lib.hasSuffix ".nix" name) (
    builtins.readDir ./.
  );

in
lib.foldlAttrs (
  acc: name: _:
  acc // callTest (./. + "/${name}")
) { } testFiles

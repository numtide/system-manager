{
  lib,
}:
{
  # Create a container test derivation
  # Arguments:
  #   hostPkgs: The host's nixpkgs
  #   name: Name of the test
  #   toplevel: system-manager profile to test
  #   testScript: Python test script
  #   extraPathsToRegister: Additional store paths to make available
  makeContainerTest =
    {
      hostPkgs,
      name,
      toplevel,
      testScript,
      extraPathsToRegister ? [ ],
    }:
    let
      testDriver = hostPkgs.callPackage ./package.nix { };
      ubuntuRootfs = import ./ubuntu-rootfs.nix { pkgs = hostPkgs; };

      # Create closure info for nix copy
      closureInfo = hostPkgs.closureInfo {
        rootPaths = [ toplevel ] ++ extraPathsToRegister;
      };

      testScriptFile = hostPkgs.writeText "test-script.py" testScript;
    in
    hostPkgs.stdenv.mkDerivation {
      name = "container-test-${name}";

      # Required for systemd-nspawn to work in the sandbox
      requiredSystemFeatures = [ "uid-range" ];

      nativeBuildInputs = [
        testDriver
        hostPkgs.util-linux
        hostPkgs.coreutils
        hostPkgs.iproute2
        hostPkgs.systemd
      ];

      passthru = {
        inherit
          toplevel
          testDriver
          ubuntuRootfs
          closureInfo
          ;
        driver = hostPkgs.writeShellScriptBin "run-container-test" ''
          exec ${testDriver}/bin/container-test-driver \
            --ubuntu-rootfs ${ubuntuRootfs} \
            --container-name ${name} \
            --profile ${toplevel} \
            --host-nix-store /nix/store \
            --closure-info ${closureInfo} \
            --test-script ${testScriptFile} \
            "$@"
        '';
      };

      buildCommand = ''
        mkdir -p $out

        # Run the container test driver
        container-test-driver \
          --ubuntu-rootfs ${ubuntuRootfs} \
          --container-name ${name} \
          --profile ${toplevel} \
          --host-nix-store /nix/store \
          --closure-info ${closureInfo} \
          --test-script ${testScriptFile} \
          -o $out

        touch $out/passed
      '';
    };
}

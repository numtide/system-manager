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
  #   skipTypeCheck: Skip mypy type checking (default: false)
  #   skipLint: Skip pyflakes linting (default: false)
  makeContainerTest =
    {
      hostPkgs,
      name,
      toplevel,
      testScript,
      extraPathsToRegister ? [ ],
      skipTypeCheck ? false,
      skipLint ? false,
    }:
    let
      testDriver = hostPkgs.callPackage ./package.nix { };
      ubuntuRootfs = import ./ubuntu-rootfs.nix { pkgs = hostPkgs; };

      # Create closure info for nix copy
      closureInfo = hostPkgs.closureInfo {
        rootPaths = [ toplevel ] ++ extraPathsToRegister;
      };

      testScriptFile = hostPkgs.writeText "test-script.py" testScript;

      # Container name converted to valid Python identifier
      pythonizedName =
        let
          head = builtins.substring 0 1 name;
          tail = builtins.substring 1 (-1) name;
        in
        (if builtins.match "[A-z_]" head == null then "_" else head)
        + lib.stringAsChars (c: if builtins.match "[A-z0-9_]" c == null then "_" else c) tail;
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
      ]
      ++ lib.optionals (!skipTypeCheck) [ hostPkgs.mypy ]
      ++ lib.optionals (!skipLint) [ hostPkgs.python3Packages.pyflakes ];

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

      testScript = testScript;

      buildCommand = ''
        mkdir -p $out
        cp ${testScriptFile} $out/test-script

      ''
      + lib.optionalString (!skipTypeCheck) ''
        # Type check the test script with mypy
        echo "Running type check"
        cat "${./test-script-prepend.py}" > testScriptWithTypes
        echo "${pythonizedName}: Machine;" >> testScriptWithTypes
        echo -n "$testScript" >> testScriptWithTypes
        mypy --no-implicit-optional --pretty --no-color-output testScriptWithTypes

      ''
      + lib.optionalString (!skipLint) ''
        # Lint the test script with pyflakes
        echo "Linting test script"
        generate-driver-symbols
        PYFLAKES_BUILTINS="$(
          echo -n ${lib.escapeShellArg pythonizedName},
          cat driver-symbols
        )" pyflakes $out/test-script

      ''
      + ''
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

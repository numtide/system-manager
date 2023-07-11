{ nixpkgs   # The nixpkgs flake
, self      # The system-manager flake
, nixos     # The path to the nixos dir from nixpkgs
,
}:
let
  inherit (nixpkgs) lib;
in
{
  # Function that can be used when defining inline modules to get better location
  # reporting in module-system errors.
  # Usage example:
  #   { _file = "${printAttrPos (builtins.unsafeGetAttrPos "a" { a = null; })}: inline module"; }
  printAttrPos = { file, line, column }: "${file}:${toString line}:${toString column}";

  makeSystemConfig =
    { modules
    , extraSpecialArgs ? { }
    }:
    let
      # Module that sets additional module arguments
      extraArgsModule = { lib, config, pkgs, ... }: {
        _file = "${self.lib.printAttrPos (builtins.unsafeGetAttrPos "a" { a = null; })}: inline module";
        _module.args = {
          pkgs = nixpkgs.legacyPackages.${config.nixpkgs.hostPlatform};
          utils = import "${nixos}/lib/utils.nix" {
            inherit lib config pkgs;
          };
          # Pass the wrapped system-manager binary down
          inherit (self.packages.${config.nixpkgs.hostPlatform}) system-manager;
        };
      };

      config = (lib.evalModules {
        specialArgs = { nixosModulesPath = "${nixos}/modules"; } // extraSpecialArgs;
        modules = [
          extraArgsModule
          ./modules
        ] ++ modules;
      }).config;

      # Get the system as it was defined in the modules.
      system = config.nixpkgs.hostPlatform;
      pkgs = nixpkgs.legacyPackages.${system};

      returnIfNoAssertions = drv:
        let
          failedAssertions = map (x: x.message) (lib.filter (x: !x.assertion) config.assertions);
        in
        if failedAssertions != [ ]
        then throw "\nFailed assertions:\n${lib.concatStringsSep "\n" (map (x: "- ${x}") failedAssertions)}"
        else lib.showWarnings config.warnings drv;

      servicesPath = pkgs.writeTextFile {
        name = "services";
        destination = "/services.json";
        text = lib.generators.toJSON { } config.build.services;
      };

      etcPath = pkgs.writeTextFile {
        name = "etcFiles";
        destination = "/etcFiles.json";
        text = lib.generators.toJSON { } { inherit (config.build.etc) entries staticEnv; };
      };

      linkFarmNestedEntryFromDrv = dirs: drv: {
        name = lib.concatStringsSep "/" (dirs ++ [ "${drv.name}" ]);
        path = drv;
      };
      linkFarmEntryFromDrv = linkFarmNestedEntryFromDrv [ ];
      linkFarmBinEntryFromDrv = linkFarmNestedEntryFromDrv [ "bin" ];

      toplevel =
        let
          scripts = lib.mapAttrsToList
            (_: script: linkFarmBinEntryFromDrv script)
            config.build.scripts;

          entries = [
            (linkFarmEntryFromDrv servicesPath)
            (linkFarmEntryFromDrv etcPath)
          ] ++ scripts;

          addPassthru = drv: drv.overrideAttrs (prevAttrs: {
            passthru = (prevAttrs.passthru or { }) // {
              inherit config;
            };
          });
        in
        addPassthru (pkgs.linkFarm "system-manager" entries);
    in
    returnIfNoAssertions toplevel;

  # TODO: put these in an external JSON file that we can automatically update
  images.ubuntu = {
    x86_64-linux = {
      ubuntu_23_04_cloudimg = {
        name = "ubuntu-23.04-server-cloudimg-amd64.img";
        releaseName = "lunar";
        releaseTimeStamp = "20230502";
        hash = "sha256-E5ZchMZcurCzQyasNKwMR6iAMPnf+A5jkeVsuQd8rdA=";
      };

      ubuntu_22_10_cloudimg = {
        name = "ubuntu-22.10-server-cloudimg-amd64.img";
        releaseName = "kinetic";
        releaseTimeStamp = "20230428";
        hash = "sha256-HYgpm243gfJgY3zK2lVVlSLfW3a/Vhdop/zJErIt6r4=";
      };

      ubuntu_22_04_cloudimg = {
        name = "ubuntu-22.04-server-cloudimg-amd64.img";
        releaseName = "jammy";
        releaseTimeStamp = "20230427";
        hash = "sha256-m76TZOKYnBzOLBZpt6kcK70TkFKHaoyBzVLA+q77ZHQ=";
      };

      ubuntu_20_04_cloudimg = {
        name = "ubuntu-20.04-server-cloudimg-amd64.img";
        releaseName = "focal";
        releaseTimeStamp = "20230420";
        hash = "sha256-XFUVWvk8O1IHfp+sAiOSCU5ASk/qJG2JIF4WH0ex12U=";
      };
    };
    aarch64-linux = {
      ubuntu_23_04_cloudimg = {
        name = "ubuntu-23.04-server-cloudimg-arm64.img";
        releaseName = "lunar";
        releaseTimeStamp = "20230502";
        hash = "";
      };

      ubuntu_22_10_cloudimg = {
        name = "ubuntu-22.10-server-cloudimg-arm64.img";
        releaseName = "kinetic";
        releaseTimeStamp = "20230428";
        hash = "";
      };

      ubuntu_22_04_cloudimg = {
        name = "ubuntu-22.04-server-cloudimg-arm64.img";
        releaseName = "jammy";
        releaseTimeStamp = "20230427";
        hash = "sha256-9vkeg5VumVBxj4TaLd0SgJEWjw11pcP7SBz5zd1V0EE=";
      };

      ubuntu_20_04_cloudimg = {
        name = "ubuntu-20.04-server-cloudimg-arm64.img";
        releaseName = "focal";
        releaseTimeStamp = "20230420";
        hash = "sha256-YUtW3oMHz4Hw7WeIu6ksx+/mUfxp7cCSSETvY6KGwU4=";
      };
    };
  };

  # Careful since we do not have the nix store yet when this service runs,
  # so we cannot use pkgs.writeTest or pkgs.writeShellScript for instance,
  # since their results would refer to the store
  mount_store = { pkgs, pathsToRegister }:
    let
      pathRegistrationInfo = "${pkgs.closureInfo { rootPaths = pathsToRegister; }}/registration";
    in
    pkgs.writeText "mount-store.service" ''
      [Service]
      Type = oneshot
      ExecStart = mkdir -p /nix/.ro-store
      ExecStart = mount -t 9p -o defaults,trans=virtio,version=9p2000.L,cache=loose,msize=${toString (256 * 1024 * 1024)} nix-store /nix/.ro-store
      ExecStart = mkdir -p -m 0755 /nix/.rw-store/ /nix/store
      ExecStart = mount -t tmpfs tmpfs /nix/.rw-store
      ExecStart = mkdir -p -m 0755 /nix/.rw-store/store /nix/.rw-store/work
      ExecStart = mount -t overlay overlay /nix/store -o lowerdir=/nix/.ro-store,upperdir=/nix/.rw-store/store,workdir=/nix/.rw-store/work

      # Register the required paths in the nix DB.
      # The store has been mounted at this point, to we can use writeShellScript now.
      ExecStart = ${pkgs.writeShellScript "execstartpost-script" ''
        ${lib.getBin pkgs.nix}/bin/nix-store --load-db < ${pathRegistrationInfo}
      ''}

      [Install]
      WantedBy = multi-user.target
    '';

  # Backdoor service that exposes a root shell through a socket to the test instrumentation framework
  backdoor = { pkgs }:
    pkgs.writeText "backdoor.service" ''
      [Unit]
      Requires = dev-hvc0.device dev-ttyS0.device mount-store.service
      After = dev-hvc0.device dev-ttyS0.device mount-store.service
      # Keep this unit active when we switch to rescue mode for instance
      IgnoreOnIsolate = true

      [Service]
      ExecStart = ${pkgs.writeShellScript "backdoor-start-script" ''
        set -euo pipefail

        export USER=root
        export HOME=/root
        export DISPLAY=:0.0

        # TODO: do we actually need to source /etc/profile ?
        # Unbound vars cause the service to crash
        #source /etc/profile

        # Don't use a pager when executing backdoor
        # actions. Because we use a tty, commands like systemctl
        # or nix-store get confused into thinking they're running
        # interactively.
        export PAGER=

        cd /tmp
        exec < /dev/hvc0 > /dev/hvc0
        while ! exec 2> /dev/ttyS0; do sleep 0.1; done
        echo "connecting to host..." >&2
        stty -F /dev/hvc0 raw -echo # prevent nl -> cr/nl conversion
        # This line is essential since it signals to the test driver that the
        # shell is ready.
        # See: the connect method in the Machine class.
        echo "Spawning backdoor root shell..."
        # Passing the terminal device makes bash run non-interactively.
        # Otherwise we get errors on the terminal because bash tries to
        # setup things like job control.
        PS1= exec /usr/bin/env bash --norc /dev/hvc0
      ''}
      KillSignal = SIGHUP

      [Install]
      WantedBy = multi-user.target
    '';

  prepareUbuntuImage = { hostPkgs, nodeConfig, image, extraPathsToRegister ? [ ] }:
    let
      pkgs = hostPkgs;

      img = pkgs.fetchurl {
        inherit (image) hash;
        url = "https://cloud-images.ubuntu.com/releases/${image.releaseName}/release-${image.releaseTimeStamp}/${image.name}";
      };

      resultImg = "./image.qcow2";

      # The nix store paths that need to be added to the nix DB for this node.
      pathsToRegister = [ nodeConfig.systemConfig ] ++ extraPathsToRegister;
    in
    pkgs.runCommand "${image.name}-system-manager-vm-test.qcow2" { } ''
      # We will modify the VM image, so we need a mutable copy
      install -m777 ${img} ${resultImg}

      # Copy the service files here, since otherwise they end up in the VM
      # with their paths including the nix hash
      cp ${self.lib.backdoor { inherit pkgs; }} backdoor.service
      cp ${self.lib.mount_store { inherit pkgs pathsToRegister; }} mount-store.service

      #export LIBGUESTFS_DEBUG=1 LIBGUESTFS_TRACE=1
      ${lib.concatStringsSep "  \\\n" [
        "${pkgs.guestfs-tools}/bin/virt-customize"
        "-a ${resultImg}"
        "--smp 2"
        "--memsize 256"
        "--no-network"
        "--copy-in backdoor.service:/etc/systemd/system"
        "--copy-in mount-store.service:/etc/systemd/system"
        ''--link ${nodeConfig.systemConfig}:/system-manager-profile''
        "--run"
        (pkgs.writeShellScript "run-script" ''
          # Clear the root password
          passwd -d root

          # Don't spawn ttys on these devices, they are used for test instrumentation
          systemctl mask serial-getty@ttyS0.service
          systemctl mask serial-getty@hvc0.service
          # Speed up the boot process
          systemctl mask snapd.service
          systemctl mask snapd.socket
          systemctl mask snapd.seeded.service

          # We have no network in the test VMs, avoid an error on bootup
          systemctl mask ssh.service
          systemctl mask ssh.socket

          systemctl enable backdoor.service
        '')
      ]};

      cp ${resultImg} $out
    '';

  mkTestPreamble =
    { node
    , profile
    , action
    }: ''
      ${node}.succeed("/${profile}/bin/${action} 2>&1 | tee /tmp/output.log")
      ${node}.succeed("! grep -F 'ERROR' /tmp/output.log")
    '';

  activateProfileSnippet = { node, profile ? "system-manager-profile" }:
    self.lib.mkTestPreamble {
      inherit node profile;
      action = "activate";
    };
  deactivateProfileSnippet = { node, profile ? "system-manager-profile" }:
    self.lib.mkTestPreamble {
      inherit node profile;
      action = "deactivate";
    };
  prepopulateProfileSnippet = { node, profile ? "system-manager-profile" }:
    self.lib.mkTestPreamble {
      inherit node profile;
      action = "prepopulate";
    };

  make-vm-test =
    name:
    { system
    , modules
    }:
    let
      hostPkgs = nixpkgs.legacyPackages.${system};

      config = (lib.evalModules {
        specialArgs = { system-manager = self; };
        modules = [
          ../test/nix/test-driver/modules
          {
            _file = "${self.lib.printAttrPos (builtins.unsafeGetAttrPos "a" { a = null; })}: inline module";
            inherit hostPkgs;
          }
        ] ++ modules;
      }).config;

      nodes = map runVmScript (lib.attrValues config.nodes);

      runVmScript = node:
        # The test driver extracts the name of the node from the name of the
        # VM script, so it's important here to stick to the naming scheme expected
        # by the test driver.
        hostPkgs.writeShellScript "run-${node.system.name}-vm" ''
          set -eo pipefail

          export PATH=${lib.makeBinPath [ hostPkgs.coreutils ]}''${PATH:+:}$PATH

          # Create a directory for storing temporary data of the running VM.
          if [ -z "$TMPDIR" ] || [ -z "$USE_TMPDIR" ]; then
            TMPDIR=$(mktemp -d nix-vm.XXXXXXXXXX --tmpdir)
          fi

          # Create a directory for exchanging data with the VM.
          mkdir -p "$TMPDIR/xchg"

          cd "$TMPDIR"

          # Start QEMU.
          # We might need to be smarter about the QEMU binary to run when we want to
          # support architectures other than x86_64.
          # See qemu-common.nix in nixpkgs.
          ${lib.concatStringsSep "\\\n  " [
            "exec ${lib.getBin hostPkgs.qemu_test}/bin/qemu-kvm"
            "-device virtio-rng-pci"
            "-cpu max"
            "-name ${node.system.name}"
            "-m ${toString node.virtualisation.memorySize}"
            "-smp ${toString node.virtualisation.cpus}"
            "-drive file=${node.virtualisation.rootImage},format=qcow2"
            "-device virtio-net-pci,netdev=net0"
            "-netdev user,id=net0"
            "-virtfs local,security_model=passthrough,id=fsdev1,path=/nix/store,readonly=on,mount_tag=nix-store"
            (lib.concatStringsSep "\\\n  "
              (lib.mapAttrsToList
                (tag: share: "-virtfs local,path=${share.source},security_model=none,mount_tag=${tag}")
                  node.virtualisation.sharedDirectories))
            "-snapshot"
            "-nographic"
            "$QEMU_OPTS"
            "$@"
          ]};
        '';

      test-driver =
        let
          upstream = hostPkgs.callPackage "${nixpkgs}/nixos/lib/test-driver" { };
        in
        upstream.overrideAttrs (_: {
          # Try to apply the patch for backwards compat.
          # It is included upstream starting from NixOS 23.05.
          # github.com/NixOS/nixpkgs#228220
          postPatch =
            let
              patch = "${lib.getBin hostPkgs.patch}/bin/patch";
              patchFile = ../test/0001-nixos-test-driver-include-a-timeout-for-the-recv-cal.patch;
            in
            ''
              echo "Try to apply patch ${patchFile}..."
              if grep --quiet --fixed-strings "bash" test_driver/machine.py; then
                echo "Patch already present, ignoring..."
              else
                ${patch} -p1 < ${patchFile}
              fi
            '';
        });

      runTest = { nodes, vlans, testScript, extraDriverArgs }: ''
        ${lib.getBin test-driver}/bin/nixos-test-driver \
          ${extraDriverArgs} \
          --start-scripts ${lib.concatStringsSep " " nodes} \
          --vlans ${lib.concatStringsSep " " vlans} \
          -- ${hostPkgs.writeText "test-script" config.testScript}
      '';

      defaultTest = { extraDriverArgs ? "" }: runTest {
        inherit extraDriverArgs nodes;
        inherit (config) testScript;
        vlans = [ "1" ];
      };
    in
    hostPkgs.stdenv.mkDerivation (finalAttrs: {
      inherit name;

      requiredSystemFeatures = [ "kvm" "nixos-test" ];

      buildCommand = ''
        ${defaultTest {}}
        touch $out
      '';

      passthru = {
        runVM = hostPkgs.writeShellScriptBin "run-vm"
          (defaultTest {
            extraDriverArgs = "--interactive";
          });
      };
    });
}

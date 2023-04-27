{ nixpkgs   # The nixpkgs flake
, self      # The system-manager flake
, nixos     # The path to the nixos dir from nixpkgs
,
}:
let
  inherit (nixpkgs) lib;
in
{
  makeSystemConfig =
    { modules
    , extraSpecialArgs ? { }
    ,
    }:
    let
      # Module that sets additional module arguments
      extraArgsModule = { lib, config, pkgs, ... }: {
        _file = "lib.nix: extraArgsModule";
        _module.args = {
          pkgs = nixpkgs.legacyPackages.${config.nixpkgs.hostPlatform};
          utils = import "${nixos}/lib/utils.nix" {
            inherit lib config pkgs;
          };
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
      inherit (self.packages.${system}) system-manager;

      returnIfNoAssertions = drv:
        let
          failedAssertions = map (x: x.message) (lib.filter (x: !x.assertion) config.assertions);
        in
        if failedAssertions != [ ]
        then throw "\nFailed assertions:\n${lib.concatStringsSep "\n" (map (x: "- ${x}") failedAssertions)}"
        else lib.showWarnings config.warnings drv;

      services =
        lib.mapAttrs'
          (unitName: unit:
            lib.nameValuePair unitName {
              storePath =
                ''${unit.unit}/${unitName}'';
            })
          config.systemd.units;

      servicesPath = pkgs.writeTextFile {
        name = "services";
        destination = "/services.json";
        text = lib.generators.toJSON { } services;
      };

      # TODO: handle globbing
      etcFiles =
        let
          addToStore = name: file: pkgs.runCommandLocal "${name}-etc-link" { } ''
            mkdir -p "$out/$(dirname "${file.target}")"
            ln -s "${file.source}" "$out/${file.target}"

            if [ "${file.mode}" != symlink ]; then
              echo "${file.mode}" > "$out/${file.target}.mode"
              echo "${file.user}" > "$out/${file.target}.uid"
              echo "${file.group}" > "$out/${file.target}.gid"
            fi
          '';

          filteredEntries = lib.filterAttrs
            (_name: etcFile: etcFile.enable)
            config.environment.etc;

          srcDrvs = lib.mapAttrs addToStore filteredEntries;

          entries = lib.mapAttrs
            (name: file: file // { source = "${srcDrvs.${name}}"; })
            filteredEntries;

          staticEnv = pkgs.buildEnv {
            name = "etc-static-env";
            paths = lib.attrValues srcDrvs;
          };
        in
        { inherit entries staticEnv; };

      etcPath = pkgs.writeTextFile {
        name = "etcFiles";
        destination = "/etcFiles.json";
        text = lib.generators.toJSON { } etcFiles;
      };

      registerProfileScript = pkgs.writeShellScript "register-profile" ''
        ${system-manager}/bin/system-manager generate \
          --store-path "$(dirname $(realpath $(dirname ''${0})))" \
          "$@"
      '';

      activationScript = pkgs.writeShellScript "activate" ''
        ${system-manager}/bin/system-manager activate \
          --store-path "$(dirname $(realpath $(dirname ''${0})))" \
          "$@"
      '';

      deactivationScript = pkgs.writeShellScript "deactivate" ''
        ${system-manager}/bin/system-manager deactivate "$@"
      '';

      preActivationAssertionScript =
        let
          mkAssertion = { name, script, ... }: ''
            # ${name}

            echo -e "Evaluating pre-activation assertion ${name}...\n"
            (
              set +e
              ${script}
            )
            assertion_result=$?

            if [ $assertion_result -ne 0 ]; then
              failed_assertions+=${name}
            fi
          '';

          mkAssertions = assertions:
            lib.concatStringsSep "\n" (
              lib.mapAttrsToList (name: mkAssertion) (
                lib.filterAttrs (name: cfg: cfg.enable)
                  assertions
              )
            );
        in
        pkgs.writeShellScript "preActivationAssertions" ''
          set -ou pipefail

          declare -a failed_assertions=()

          ${mkAssertions config.system-manager.preActivationAssertions}

          if [ ''${#failed_assertions[@]} -ne 0 ]; then
            for failed_assertion in ''${failed_assertions[@]}; do
              echo "Pre-activation assertion $failed_assertion failed."
            done
            echo "See the output above for more details."
            exit 1
          else
            echo "All pre-activation assertions succeeded."
            exit 0
          fi
        '';

      linkFarmNestedEntryFromDrv = dirs: drv: {
        name = lib.concatStringsSep "/" (dirs ++ [ "${drv.name}" ]);
        path = drv;
      };
      linkFarmEntryFromDrv = linkFarmNestedEntryFromDrv [ ];
      linkFarmBinEntryFromDrv = linkFarmNestedEntryFromDrv [ "bin" ];
    in
    returnIfNoAssertions (
      pkgs.linkFarm "system-manager" [
        (linkFarmEntryFromDrv servicesPath)
        (linkFarmEntryFromDrv etcPath)
        (linkFarmBinEntryFromDrv activationScript)
        (linkFarmBinEntryFromDrv deactivationScript)
        (linkFarmBinEntryFromDrv registerProfileScript)
        (linkFarmBinEntryFromDrv preActivationAssertionScript)
      ]
    );

  # TODO: put these in an external JSON file that we can automatically update
  images.ubuntu = {
    x86_64-linux = {
      ubuntu_22_10_cloudimg = {
        name = "kinetic-server-cloudimg-amd64.img";
        releaseName = "kinetic";
        releaseTimeStamp = "20230424";
        hash = "sha256-54LucfgXtNAxKKQKmvHCk8EzPRlULGq/IfUjAvUaOXk=";
      };

      ubuntu_22_04_cloudimg = {
        name = "jammy-server-cloudimg-amd64.img";
        releaseName = "jammy";
        releaseTimeStamp = "20230427";
        hash = "sha256-m76TZOKYnBzOLBZpt6kcK70TkFKHaoyBzVLA+q77ZHQ=";
      };

      ubuntu_20_04_cloudimg = {
        name = "focal-server-cloudimg-amd64.img";
        releaseName = "focal";
        releaseTimeStamp = "20230420";
        hash = "sha256-XFUVWvk8O1IHfp+sAiOSCU5ASk/qJG2JIF4WH0ex12U=";
      };
    };
    aarch64-linux = {
      ubuntu_22_10_cloudimg = {
        name = "kinetic-server-cloudimg-arm64.img";
        releaseName = "kinetic";
        releaseTimeStamp = "20230424";
        hash = "sha256-AS8bXXqWwJdlKUYxI1MO48AyWR++Ttf1+C7ahicKiks=";
      };

      ubuntu_22_04_cloudimg = {
        name = "jammy-server-cloudimg-arm64.img";
        releaseName = "jammy";
        releaseTimeStamp = "20230427";
        hash = "sha256-9vkeg5VumVBxj4TaLd0SgJEWjw11pcP7SBz5zd1V0EE=";
      };

      ubuntu_20_04_cloudimg = {
        name = "focal-server-cloudimg-arm64.img";
        releaseName = "focal";
        releaseTimeStamp = "20230420";
        hash = "sha256-YUtW3oMHz4Hw7WeIu6ksx+/mUfxp7cCSSETvY6KGwU4=";
      };
    };
  };

  # Careful since we do not have the nix store yet when this service runs,
  # so we cannot use pkgs.writeTest or pkgs.writeShellScript for instance,
  # since their results would refer to the store
  mount_store = { pkgs }:
    pkgs.writeText "mount-store.service" ''
      [Service]
      Type = oneshot
      ExecStart = mkdir -p /nix/.ro-store
      ExecStart = mount -t 9p -o defaults,trans=virtio,version=9p2000.L,cache=loose,msize=${toString (256 * 1024 * 1024)} nix-store /nix/.ro-store
      ExecStart = mkdir -p -m 0755 /nix/.rw-store/ /nix/store
      ExecStart = mount -t tmpfs tmpfs /nix/.rw-store
      ExecStart = mkdir -p -m 0755 /nix/.rw-store/store /nix/.rw-store/work
      ExecStart = mount -t overlay overlay /nix/store -o lowerdir=/nix/.ro-store,upperdir=/nix/.rw-store/store,workdir=/nix/.rw-store/work

      [Install]
      WantedBy = multi-user.target
    '';

  # Backdoor service that exposes a root shell through a socket to the test instrumentation framework
  backdoor = { pkgs }:
    pkgs.writeText "backdoor.service" ''
      [Unit]
      Requires = dev-hvc0.device dev-ttyS0.device mount-store.service
      After = dev-hvc0.device dev-ttyS0.device mount-store.service

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

  prepareUbuntuImage = { hostPkgs, nodeConfig, image }:
    let
      pkgs = hostPkgs;

      guestfs-tools = pkgs.guestfs-tools.overrideAttrs (_: {
        doCheck = false;
        doInstallCheck = false;
      });

      img = pkgs.fetchurl {
        inherit (image) hash;
        url = "https://cloud-images.ubuntu.com/${image.releaseName}/${image.releaseTimeStamp}/${image.name}";
      };
    in
    pkgs.runCommand "configure-vm" { } ''
      # We will modify the VM image, so we need a mutable copy
      install -m777 ${img} ./img.qcow2

      # Copy the service files here, since otherwise they end up in the VM
      # wwith their paths including the nix hash
      cp ${self.lib.backdoor { inherit pkgs; }} backdoor.service
      cp ${self.lib.mount_store { inherit pkgs; }} mount-store.service

      ${lib.concatStringsSep "  \\\n" [
        "${guestfs-tools}/bin/virt-customize"
        "-a ./img.qcow2"
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

          systemctl enable backdoor.service
        '')
      ]};

      cp ./img.qcow2 $out
    '';

  make-vm-test =
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
            _file = "inline module in lib.nix";
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
          # github.com/NixOS/nixpkgs#228220 gets merged
          patches = [
            ../test/0001-nixos-test-driver-include-a-timeout-for-the-recv-cal.patch
          ];
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
      name = "system-manager-vm-test";

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

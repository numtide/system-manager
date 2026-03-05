# Container tests using systemd-nspawn
{
  lib,
  system-manager,
  system,
  hostPkgs,
  nixpkgs,
}:

let
  containerTestLib = import ../lib/container-test-driver { inherit lib; };

  # Helper to create a container test for a system-manager configuration
  makeContainerTestFor =
    name:
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
              };
            }
          )
        ];
      };
    in
    containerTestLib.makeContainerTest {
      inherit hostPkgs name;
      inherit toplevel;
      testScript = testScriptFunction { inherit toplevel hostPkgs; };
      extraPathsToRegister = extraPathsToRegister ++ [ toplevel ];
    };

in

{
  container-example = makeContainerTestFor "example" {
    modules = [
      ../examples/example.nix
    ];
    testScriptFunction =
      { toplevel, hostPkgs, ... }:
      ''
        # Start the container
        start_all()

        # Wait for Ubuntu systemd to be ready
        machine.wait_for_unit("multi-user.target")
        # System manager is trying to configure nix. Activation will get a partial error if we do not delete it.
        machine.execute("rm -rf /etc/nix")

        # Nix is installed and profile is copied by the driver automatically
        # Activate system-manager
        def activate_and_check():
          activation_logs = machine.activate()
          for line in activation_logs.split("\n"):
            assert not "ERROR" in line, line
          machine.wait_for_unit("system-manager.target")

          with subtest("Verify services are running"):
              assert machine.service("nginx").is_running, "nginx should be running"
              assert machine.service("service-0").is_enabled, "service-0 should be enabled"
              assert machine.service("service-9").is_enabled, "service-9 should be enabled"

          with subtest("Verify packages are in PATH"):
              machine.succeed("bash --login -c 'which rg'")
              machine.succeed("bash --login -c 'which fd'")

          with subtest("Verify /etc/foo.conf configuration"):
              foo_conf = machine.file("/etc/foo.conf")
              assert foo_conf.exists, "/etc/foo.conf should exist"
              assert foo_conf.is_file, "/etc/foo.conf should be a file"
              assert foo_conf.contains("launch_the_rockets = true"), "foo.conf should contain launch_the_rockets = true"
              assert not foo_conf.contains("launch_the_rockets = false"), "foo.conf should not contain launch_the_rockets = false"

          with subtest("Verify symlinks"):
              foo2 = machine.file("/etc/baz/bar/foo2")
              assert foo2.is_symlink, "/etc/baz/bar/foo2 should be a symlink"
              assert foo2.exists, "/etc/baz/bar/foo2 should exist (symlink target valid)"

          with subtest("Verify nested directories"):
              assert machine.file("/etc/a/nested/example").is_directory, "/etc/a/nested/example should be a directory"
              assert machine.file("/etc/a/nested/example/foo3").is_file, "/etc/a/nested/example/foo3 should be a file"
              assert machine.file("/etc/a/nested/example2").is_directory, "/etc/a/nested/example2 should be a directory"

          with subtest("Verify file ownership by uid/gid"):
              with_ownership = machine.file("/etc/with_ownership")
              assert with_ownership.uid == 5, f"uid was {with_ownership.uid}, expected 5"
              assert with_ownership.gid == 6, f"gid was {with_ownership.gid}, expected 6"

          with subtest("Verify file ownership by user/group name"):
              with_ownership2 = machine.file("/etc/with_ownership2")
              assert with_ownership2.user == "nobody", f"user was {with_ownership2.user}, expected nobody"
              assert with_ownership2.group == "users", f"group was {with_ownership2.group}, expected users"

          with subtest("Verify tmpfiles directories"):
              assert machine.file("/var/tmp/system-manager").is_directory, "/var/tmp/system-manager should be a directory"
              assert machine.file("/var/tmp/sample").is_directory, "/var/tmp/sample should be a directory"

          with subtest("Verify tmpfiles.d configurations"):
              assert machine.file("/etc/tmpfiles.d/sample.conf").is_file, "sample.conf should exist"
              assert machine.file("/etc/tmpfiles.d/00-system-manager.conf").is_file, "00-system-manager.conf should exist"

        activate_and_check()
        activate_and_check()
      '';
  };

  container-extra-init = makeContainerTestFor "extra-init" {
    modules = [
      (
        { ... }:
        {
          environment.etc."nix/nix.conf".replaceExisting = true;

          environment.extraInit = ''
            export MY_CUSTOM_VAR="hello-from-extraInit"
          '';
        }
      )
    ];
    testScriptFunction =
      { toplevel, hostPkgs, ... }:
      ''
        start_all()

        machine.wait_for_unit("multi-user.target")

        activation_logs = machine.activate()
        for line in activation_logs.split("\n"):
            assert not "ERROR" in line, line
        machine.wait_for_unit("system-manager.target")

        with subtest("extraInit code is present in profile script"):
            content = machine.succeed("cat /etc/profile.d/system-manager-path.sh")
            assert "MY_CUSTOM_VAR" in content, f"Expected extraInit content in profile script, got: {content}"

        with subtest("extraInit variable is set in login shell"):
            value = machine.succeed("bash --login -c 'echo $MY_CUSTOM_VAR'").strip()
            assert value == "hello-from-extraInit", f"Expected 'hello-from-extraInit', got: '{value}'"
      '';
  };

  container-masked-units = makeContainerTestFor "masked-units" {
    modules = [
      (
        { ... }:
        {
          systemd.maskedUnits = [ "unattended-upgrades.service" ];
        }
      )
      ../examples/example.nix
    ];
    testScriptFunction =
      { toplevel, hostPkgs, ... }:
      ''
        start_all()

        machine.wait_for_unit("multi-user.target")

        with subtest("Service is not masked before activation"):
            machine.fail("test -L /etc/systemd/system/unattended-upgrades.service")

        with subtest("Service can be started before activation"):
            assert machine.service("unattended-upgrades").is_running, "unattended-upgrades should be running before activation"

        machine.activate()
        machine.wait_for_unit("system-manager.target")

        with subtest("Masked service is not running"):
            assert not machine.service("unattended-upgrades").is_running, "unattended-upgrades should not be running"

        with subtest("Service is masked after activation"):
            resolved = machine.succeed("readlink -f /etc/systemd/system/unattended-upgrades.service").strip()
            assert resolved == "/dev/null", f"expected /dev/null, got {resolved}"

        with subtest("Masked service cannot be started"):
            machine.fail("systemctl start unattended-upgrades.service")

        with subtest("Deactivation unmasks the service"):
            machine.succeed("${toplevel}/bin/deactivate")
            machine.fail("test -L /etc/systemd/system/unattended-upgrades.service")
      '';
  };
  container-etc-files-with-glob = makeContainerTestFor "etc-files-with-glob" {
    modules = [
      (
        { pkgs, ... }:
        {
          environment.etc = {
            "fail2ban/action.d".source = "${pkgs.fail2ban}/etc/fail2ban/action.d/*.conf";
            "fail2ban/filter.d".source = "${pkgs.fail2ban}/etc/fail2ban/filter.d/*.conf";
          };
        }
      )
    ];

    testScriptFunction =
      { toplevel, hostPkgs, ... }:
      ''
        start_all()

        machine.wait_for_unit("multi-user.target")

        with subtest("File from glob is not present"):
            machine.fail("test -f /etc/fail2ban/action.d/dummy.conf")

        machine.activate()

        with subtest("File from glob is present"):
            machine.succeed("test -f /etc/fail2ban/action.d/dummy.conf")
      '';
  };

  container-system-checks =
    let
      failingToplevel = system-manager.lib.makeSystemConfig {
        modules = [
          (
            { pkgs, ... }:
            {
              nixpkgs.hostPlatform = system;
              system.checks = [
                (pkgs.runCommand "failing-check" { } ''
                  echo "this check should fail" >&2
                  exit 1
                '')
              ];
            }
          )
        ];
      };
    in
    makeContainerTestFor "system-checks" {
      modules = [
        (
          { pkgs, ... }:
          {
            system.checks = [
              (pkgs.runCommand "passing-check" { } ''
                echo "check passed" > $out
              '')
            ];
          }
        )
      ];
      extraPathsToRegister = [ ];
      testScriptFunction =
        { toplevel, hostPkgs, ... }:
        ''
          start_all()

          machine.wait_for_unit("multi-user.target")

          with subtest("Check outputs exist in toplevel under checks/"):
              machine.succeed("test -d ${toplevel}/checks")
              # Find the passing-check entry (index depends on other modules adding checks)
              machine.succeed("ls ${toplevel}/checks/ | grep -F passing-check")
              content = machine.succeed("cat ${toplevel}/checks/*-passing-check").strip()
              assert content == "check passed", f"Expected 'check passed', got: {content}"

          with subtest("Failing check prevents toplevel from building"):
              machine.fail("nix-store --realise ${builtins.unsafeDiscardOutputDependency failingToplevel.drvPath}")
        '';
    };

  container-security-wrappers = makeContainerTestFor "security-wrappers" {
    modules = [
      (
        { pkgs, lib, ... }:
        {
          environment.etc."nix/nix.conf".replaceExisting = true;

          security.wrappers.ping = {
            owner = "root";
            group = "root";
            capabilities = "cap_net_raw+ep";
            source = "${pkgs.iputils.out}/bin/ping";
          };
        }
      )
    ];
    testScriptFunction =
      { toplevel, hostPkgs, ... }:
      ''
        start_all()

        machine.wait_for_unit("multi-user.target")

        activation_logs = machine.activate()
        for line in activation_logs.split("\n"):
            assert not "ERROR" in line, line
        machine.wait_for_unit("system-manager.target")

        with subtest("suid-sgid-wrappers.service completed successfully"):
            journal = machine.succeed("journalctl -u suid-sgid-wrappers.service --no-pager").strip()
            print(journal)
            result = machine.succeed("systemctl show suid-sgid-wrappers.service -p Result --value").strip()
            assert result == "success", f"suid-sgid-wrappers.service Result={result}\n{journal}"

        with subtest("Tmpfs mount at /run/wrappers exists"):
            mount_output = machine.succeed("findmnt -n -o FSTYPE /run/wrappers").strip()
            assert mount_output == "tmpfs", f"Expected tmpfs, got: {mount_output}"

        with subtest("Wrapper binary exists and is executable"):
            wrapper = machine.file("/run/wrappers/bin/ping")
            assert wrapper.exists, "/run/wrappers/bin/ping should exist"
            assert wrapper.is_file, "/run/wrappers/bin/ping should be a file"
            machine.succeed("test -x /run/wrappers/bin/ping")

        with subtest("Wrapper binary has correct ownership"):
            owner = machine.succeed("stat -c '%U:%G' /run/wrappers/bin/ping").strip()
            assert owner == "root:root", f"Expected root:root ownership, got: {owner}"

        with subtest("Capabilities are set on wrapper binary"):
            caps = machine.succeed("getcap /run/wrappers/bin/ping").strip()
            assert "cap_net_raw" in caps, f"Expected cap_net_raw in capabilities, got: {caps}"

        with subtest("/run/wrappers/bin precedes /usr/bin in PATH"):
            path = machine.succeed("bash --login -c 'echo $PATH'").strip()
            entries = path.split(":")
            wrappers_idx = next(i for i, e in enumerate(entries) if e == "/run/wrappers/bin")
            usr_bin_idx = next(i for i, e in enumerate(entries) if e == "/usr/bin")
            assert wrappers_idx < usr_bin_idx, f"/run/wrappers/bin (index {wrappers_idx}) must come before /usr/bin (index {usr_bin_idx}) in PATH: {path}"

        with subtest("Default mount and umount wrappers exist"):
            assert machine.file("/run/wrappers/bin/mount").exists, "mount wrapper should exist"
            assert machine.file("/run/wrappers/bin/umount").exists, "umount wrapper should exist"

        with subtest("Build-time check output exists in toplevel"):
            machine.succeed("test -d ${toplevel}/checks")

        with subtest("Deactivation cleans up wrappers"):
            machine.succeed("${toplevel}/bin/deactivate")
            machine.fail("test -e /etc/systemd/system/suid-sgid-wrappers.service")
      '';
  };

  container-systemd-packages = makeContainerTestFor "systemd-packages" {
    modules = [
      (
        { pkgs, ... }:
        {
          imports = [ "${nixpkgs}/nixos/modules/services/security/fail2ban.nix" ];
          config = {

            # Enabling fail2ban to test systemd units overrides and
            # systemd.packages options.
            services.fail2ban = {
              enable = true;
              bantime = "3600";
              packageFirewall = pkgs.nftables;
            };
            networking.nftables.enable = true;

            systemd.services.fail2ban = {
              wantedBy = lib.mkForce [
                "system-manager.target"
              ];
            };
          };
          options = {
            # Dummy valies to enable fail2ban
            services.openssh.settings = lib.mkOption {
              type = lib.types.attrs;
            };
            # Some goes for nftables
            networking.nftables.enable = lib.mkEnableOption "dummy nftable module";
          };

        }
      )
    ];
    testScriptFunction =
      { toplevel, hostPkgs, ... }:
      ''
        start_all()

        machine.wait_for_unit("multi-user.target")

        machine.activate()
        machine.wait_for_unit("system-manager.target")

        with subtest("Unit file from systemd.packages is present"):
            unit = machine.file("/etc/systemd/system/fail2ban.service")
            assert unit.exists, "fail2ban.service unit file should exist"
            assert unit.is_symlink or unit.is_file, "fail2ban.service should be a file or symlink"
            machine.wait_for_unit("fail2ban.service")
      '';
  };
}

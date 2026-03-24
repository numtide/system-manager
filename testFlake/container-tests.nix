# Container tests using systemd-nspawn.
{
  lib,
  system-manager,
  system,
  hostPkgs,
  nixpkgs,
}:

let
  containerTestLib = import ../lib/container-test-driver { inherit lib; };
  distros = import ../lib/container-test-driver/distros.nix { pkgs = hostPkgs; };
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

in

forEachDistro "example" {
  modules = [
    ../examples/example.nix
  ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      # Start the container
      start_all()

      # Wait for systemd to be ready
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

      with subtest("the sshd config is not managed by system-manager when the module is not explicitely enabled"):
          try:
              assert not machine.service("ssh-system-manager.service").is_enabled, "the system manager ssh service should not be running"
          except:
              print("ssh-system-manager.service is not available as expected")

      activate_and_check()
    '';
}

// forEachDistro "extra-init" {
  modules = [
    (
      { ... }:
      {
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
}

// forEachDistro "masked-units" {
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
}

// forEachDistro "etc-files-with-glob" {
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
}

// (
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
  forEachDistro "system-checks" {
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
  }
)

// forEachDistro "nix-enabled" {
  modules = [
    (
      { ... }:
      {
        nix.enable = true;
      }
    )
  ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      start_all()

      machine.wait_for_unit("multi-user.target")

      with subtest("Pre-existing nix.conf before activation"):
          assert machine.file("/etc/nix/nix.conf").exists, "/etc/nix/nix.conf should exist before activation"
          original_nix_conf = machine.succeed("cat /etc/nix/nix.conf")

      machine.activate()
      machine.wait_for_unit("system-manager.target")

      with subtest("nix.conf is managed after activation"):
          nix_conf = machine.file("/etc/nix/nix.conf")
          assert nix_conf.exists, "/etc/nix/nix.conf should exist"
          assert nix_conf.contains("experimental-features"), "nix.conf should contain experimental-features"
          assert nix_conf.contains("nix-command"), "nix.conf should contain nix-command"
          assert nix_conf.contains("flakes"), "nix.conf should contain flakes"

      with subtest("Re-activation succeeds"):
          machine.activate()
          machine.wait_for_unit("system-manager.target")
          nix_conf = machine.file("/etc/nix/nix.conf")
          assert nix_conf.exists, "/etc/nix/nix.conf should still exist after re-activation"
          assert nix_conf.contains("flakes"), "nix.conf should still contain flakes"

      with subtest("Deactivation restores original nix.conf"):
          machine.succeed("${toplevel}/bin/deactivate")
          restored_nix_conf = machine.succeed("cat /etc/nix/nix.conf")
          assert restored_nix_conf == original_nix_conf, f"nix.conf content differs after deactivation:\n  original: {original_nix_conf!r}\n  restored: {restored_nix_conf!r}"
    '';
}

// forEachDistro "nginx-dhparams" {
  modules = [
    (
      { ... }:
      {
        services.nginx = {
          enable = true;
          sslDhparam = true;
          virtualHosts."localhost" = {
            root = "/var/www";
            locations."/".extraConfig = ''
              return 200 "ok";
            '';
          };
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

      with subtest("Verify nginx is running"):
          assert machine.service("nginx").is_running, "nginx should be running"
    '';
}

// forEachDistro "ssh" {
  modules = [
    (
      { pkgs, ... }:
      {
        programs.ssh.enable = true;
        services.openssh.enable = true;

        programs.ssh.knownHosts = {
          "github.com" = {
            publicKey = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl";
          };
          "gitlab.com" = {
            extraHostNames = [
              "gitlab.com"
              "10.0.0.1"
            ];
            publicKey = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAfuCHKVTjquxvt6CM6tdG4SLp1Btn/nOeHHE5UOzRdf";
          };
        };

        # Meh. Not great, but that's the only way I found to share this with the container.
        environment.etc."privatekey" = {
          source = pkgs.writeText "private-key" ''
            -----BEGIN OPENSSH PRIVATE KEY-----
            b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
            QyNTUxOQAAACDTaOxU7gQHrj8hPQks0u4tiVmRhF1oBAl5+2EkQ9fYBAAAAJh3B3i6dwd4
            ugAAAAtzc2gtZWQyNTUxOQAAACDTaOxU7gQHrj8hPQks0u4tiVmRhF1oBAl5+2EkQ9fYBA
            AAAEBc2BENaT8wrgOp3DsEbvS2Lt0NeTrfVztH9NLLPIE1r9No7FTuBAeuPyE9CSzS7i2J
            WZGEXWgECXn7YSRD19gEAAAAD3BpY25vaXJAYW5hcnJlcwECAwQFBg==
            -----END OPENSSH PRIVATE KEY-----
          '';
          mode = "600";
          user = "root";
          group = "root";
        };
        users.users.root.openssh.authorizedKeys.keys = [
          "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINNo7FTuBAeuPyE9CSzS7i2JWZGEXWgECXn7YSRD19gE"
        ];
      }

    )
  ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      start_all()

      machine.wait_for_unit("multi-user.target")

      # For some reason, the ubuntu image is lacking the ssh host key.
      # It's generated as a postinstall hook, so let's run it again.
      machine.succeed("dpkg-reconfigure openssh-server")

      activation_logs = machine.activate()
      for line in activation_logs.split("\n"):
          assert not "ERROR" in line, line
      machine.wait_for_unit("system-manager.target")

      with subtest("ssh_known_hosts file exists"):
          known_hosts = machine.file("/etc/ssh/ssh_known_hosts")
          assert known_hosts.exists, "/etc/ssh/ssh_known_hosts should exist"

      with subtest("github.com key is present"):
          content = machine.succeed("cat /etc/ssh/ssh_known_hosts")
          assert "github.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl" in content, \
              f"Expected github.com key in known_hosts, got: {content}"

      with subtest("gitlab.com key with extra hostnames is present"):
          content = machine.succeed("cat /etc/ssh/ssh_known_hosts")
          assert "gitlab.com,10.0.0.1 ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAfuCHKVTjquxvt6CM6tdG4SLp1Btn/nOeHHE5UOzRdf" in content, \
              f"Expected gitlab.com key with extra hostnames in known_hosts, got: {content}"

      with subtest("ssh_config references known hosts file"):
          ssh_config = machine.file("/etc/ssh/ssh_config")
          assert ssh_config.exists, "/etc/ssh/ssh_config should exist"
          config_content = machine.succeed("cat /etc/ssh/ssh_config")
          assert "GlobalKnownHostsFile" in config_content, \
              f"Expected GlobalKnownHostsFile in ssh_config, got: {config_content}"
          assert "/etc/ssh/ssh_known_hosts" in config_content, \
              f"Expected /etc/ssh/ssh_known_hosts path in ssh_config, got: {config_content}"

      with subtest("ssh server test"):
          machine.wait_for_unit("ssh-system-manager.service")
          sshd_config = machine.file("/etc/ssh/sshd_config")
          assert sshd_config.exists, "/etc/ssh/sshd_config should exist"
          sshd_content = machine.succeed("cat /etc/ssh/sshd_config")
          assert "Subsystem sftp /nix/store/" in sshd_content, \
              "/etc/ssh/sshd_config does not appear to be the system-manager provided one."
          machine.succeed('ssh -i /etc/privatekey -o "StrictHostKeyChecking no" root@localhost echo ok')
          machine.succeed('echo "ls /" | sftp -i /etc/privatekey root@localhost')

      with subtest("deactivation removes known hosts file"):
          machine.succeed("${toplevel}/bin/deactivate")
          machine.fail("test -f /etc/ssh/ssh_known_hosts")
    '';
}

// forEachDistro "empty-config" {
  modules = [ ];
  testScriptFunction =
    { toplevel, hostPkgs, ... }:
    ''
      import json

      start_all()

      machine.wait_for_unit("multi-user.target")

      with subtest("etcFiles.json contains only system-manager infrastructure"):
          etc_json = json.loads(machine.succeed("cat ${toplevel}/etcFiles/etcFiles.json"))
          etc_entries = set(etc_json["entries"].keys())
          allowed_entries = {
              "profile.d/system-manager-path.sh",
              "systemd/system",
              "tmpfiles.d",
          }
          unexpected_etc = etc_entries - allowed_entries
          assert not unexpected_etc, \
              f"Empty config should not produce etc entries beyond infrastructure: {unexpected_etc}"

      with subtest("services.json contains only system-manager infrastructure"):
          svc_json = json.loads(machine.succeed("cat ${toplevel}/services/services.json"))
          service_names = set(svc_json.keys())
          allowed_services = {
              "system-manager.target",
              "sysinit-reactivation.target",
              "system-manager-path.service",
              "userborn.service",
              "suid-sgid-wrappers.service",
              "run-wrappers.mount",
          }
          unexpected_svc = service_names - allowed_services
          assert not unexpected_svc, \
              f"Empty config should not produce services beyond infrastructure: {unexpected_svc}"
          # No service should be masked
          for name, config in svc_json.items():
              assert not config.get("masked", False), \
                  f"Infrastructure service {name} should not be masked"

      with subtest("No etc entries replace existing files"):
          etc_json = json.loads(machine.succeed("cat ${toplevel}/etcFiles/etcFiles.json"))
          for name, entry in etc_json["entries"].items():
              assert not entry.get("replaceExisting", False), \
                  f"etc entry {name} should not replace existing files in empty config"

      # Exhaustive list of paths the empty config is allowed to add or modify.
      # update this list only after confirming the change is intentional.
      allowed_changes = {
          "profile.d/system-manager-path.sh",
          # systemd unit files
          "systemd/system/system-manager.target",
          "systemd/system/sysinit-reactivation.target",
          "systemd/system/system-manager-path.service",
          "systemd/system/userborn.service",
          "systemd/system/suid-sgid-wrappers.service",
          "systemd/system/run-wrappers.mount",
          "systemd/system/default.target.wants/system-manager.target",
          "systemd/system/system-manager.target.wants/suid-sgid-wrappers.service",
          "systemd/system/system-manager.target.wants/system-manager-path.service",
          "systemd/system/sysinit-reactivation.target.requires/userborn.service",
          "systemd/system/sysinit.target.wants/userborn.service",
          # tmpfiles
          "tmpfiles.d/00-system-manager.conf",
          "tmpfiles.d/home-directories.conf",
          # userborn always touches passwd/group/shadow even with empty config
          "passwd",
          "group",
          "shadow",
          # mtab points to /proc/self/mounts, content changes with run-wrappers.mount
          "mtab",
          # resolv.conf is a dangling symlink in containers
          "resolv.conf",
      }

      def is_expected(path: str) -> bool:
          return path in allowed_changes

      def snapshot_etc() -> dict[str, str]:
          # sha256sum output: "hash  /etc/path"
          output = machine.succeed("find -L /etc -not -path '/etc/.system-manager-static/*' -not -type d -exec sha256sum {} + 2>/dev/null || true")
          snapshot: dict[str, str] = {}
          for line in output.strip().split("\n"):
              if not line:
                  continue
              sha, filepath = line.split("  ", 1)
              rel = filepath.removeprefix("/etc/")
              snapshot[rel] = sha
          return snapshot

      with subtest("Snapshot /etc before activation"):
          before = snapshot_etc()

      activation_logs = machine.activate()
      with subtest("Activation produces no errors"):
          for line in activation_logs.split("\n"):
              assert "ERROR" not in line, f"Unexpected error in activation: {line}"

      machine.wait_for_unit("system-manager.target")

      with subtest("Static environment symlink exists"):
          assert machine.file("/etc/.system-manager-static").is_symlink, \
              "/etc/.system-manager-static should be a symlink after activation"

      with subtest("No unexpected changes to /etc after activation"):
          after = snapshot_etc()
          added = [p for p in (set(after) - set(before)) if not is_expected(p)]
          removed = [p for p in (set(before) - set(after)) if not is_expected(p)]
          changed = [p for p in set(before) & set(after) if before[p] != after[p] and not is_expected(p)]
          assert not added, f"Unexpected new files in /etc: {added}"
          assert not removed, f"Files unexpectedly removed from /etc: {removed}"
          assert not changed, f"Unexpected modified files in /etc: {changed}"

      with subtest("Deactivation restores original state exactly"):
          machine.succeed("${toplevel}/bin/deactivate")
          restored = snapshot_etc()
          # userborn changes to passwd/group/shadow are not reversible
          userborn_files = {"passwd", "passwd-", "group", "group-", "shadow", "shadow-"}
          added = [p for p in (set(restored) - set(before)) if p not in userborn_files]
          removed = [p for p in (set(before) - set(restored)) if p not in userborn_files]
          changed = [p for p in set(before) & set(restored) if before[p] != restored[p] and p not in userborn_files]
          assert not added, f"Deactivation left new files: {added}"
          assert not removed, f"Deactivation removed files: {removed}"
          assert not changed, f"Deactivation left modified files: {changed}"
    '';
}

// forEachDistro "systemd-packages" {
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
        };
        options = {
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
}

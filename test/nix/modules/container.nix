# Container tests using systemd-nspawn
{
  lib,
  system-manager,
  system,
  hostPkgs,
}:

let
  containerTestLib = import ../../../lib/container-test-driver { inherit lib; };

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
      ../../../examples/example.nix
    ];
    testScriptFunction =
      { toplevel, hostPkgs, ... }:
      ''
        # Start the container
        start_all()

        # Wait for Ubuntu systemd to be ready
        machine.wait_for_unit("multi-user.target")

        # Nix is installed and profile is copied by the driver automatically
        # Activate system-manager
        machine.activate()
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
      '';
  };
}

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
              config.hostPkgs = pkgs;
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

        # Verify services using testinfra
        assert machine.service("nginx").is_running, "nginx should be running"
        assert machine.service("service-0").is_enabled, "service-0 should be enabled"
        assert machine.service("service-9").is_enabled, "service-9 should be enabled"

        # Verify packages are in PATH
        machine.succeed("bash --login -c 'which rg'")
        machine.succeed("bash --login -c 'which fd'")

        # Verify /etc files using testinfra
        foo_conf = machine.file("/etc/foo.conf")
        assert foo_conf.exists, "/etc/foo.conf should exist"
        assert foo_conf.is_file, "/etc/foo.conf should be a file"
        assert foo_conf.contains("launch_the_rockets = true"), "foo.conf should contain launch_the_rockets = true"
        assert not foo_conf.contains("launch_the_rockets = false"), "foo.conf should not contain launch_the_rockets = false"

        # Verify symlinks using testinfra
        foo2 = machine.file("/etc/baz/bar/foo2")
        assert foo2.is_symlink, "/etc/baz/bar/foo2 should be a symlink"
        assert foo2.exists, "/etc/baz/bar/foo2 should exist (symlink target valid)"

        # Verify nested directories using testinfra
        assert machine.file("/etc/a/nested/example").is_directory, "/etc/a/nested/example should be a directory"
        assert machine.file("/etc/a/nested/example/foo3").is_file, "/etc/a/nested/example/foo3 should be a file"
        assert machine.file("/etc/a/nested/example2").is_directory, "/etc/a/nested/example2 should be a directory"

        # Verify file ownership using testinfra
        with_ownership = machine.file("/etc/with_ownership")
        assert with_ownership.uid == 5, f"uid was {with_ownership.uid}, expected 5"
        assert with_ownership.gid == 6, f"gid was {with_ownership.gid}, expected 6"

        with_ownership2 = machine.file("/etc/with_ownership2")
        assert with_ownership2.user == "nobody", f"user was {with_ownership2.user}, expected nobody"
        assert with_ownership2.group == "users", f"group was {with_ownership2.group}, expected users"

        # Verify tmpfiles directories using testinfra
        assert machine.file("/var/tmp/system-manager").is_directory, "/var/tmp/system-manager should be a directory"
        assert machine.file("/var/tmp/sample").is_directory, "/var/tmp/sample should be a directory"

        # Verify tmpfiles.d configurations using testinfra
        assert machine.file("/etc/tmpfiles.d/sample.conf").is_file, "sample.conf should exist"
        assert machine.file("/etc/tmpfiles.d/00-system-manager.conf").is_file, "00-system-manager.conf should exist"
      '';
  };
}

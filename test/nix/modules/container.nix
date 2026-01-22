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
        # Now activate system-manager
        machine.succeed("${toplevel}/bin/activate")
        machine.wait_for_unit("system-manager.target")

        # Verify nginx service is running
        machine.succeed("systemctl is-active nginx")

        # Verify test services (oneshots, should have completed successfully)
        machine.succeed("systemctl is-active service-0")
        machine.succeed("systemctl is-active service-9")

        # Verify packages are in PATH
        machine.succeed("bash --login -c 'which rg'")
        machine.succeed("bash --login -c 'which fd'")

        # Verify /etc files
        machine.succeed("test -f /etc/foo.conf")
        machine.succeed("grep -F 'launch_the_rockets = true' /etc/foo.conf")
        machine.fail("grep -F 'launch_the_rockets = false' /etc/foo.conf")

        # Verify symlinks
        machine.succeed("test -L /etc/baz/bar/foo2")
        machine.succeed("test -f /etc/baz/bar/foo2")

        # Verify nested directories
        machine.succeed("test -d /etc/a/nested/example")
        machine.succeed("test -f /etc/a/nested/example/foo3")
        machine.succeed("test -d /etc/a/nested/example2")

        # Verify file ownership
        uid = machine.succeed("stat -c %u /etc/with_ownership").strip()
        gid = machine.succeed("stat -c %g /etc/with_ownership").strip()
        assert uid == "5", f"uid was {uid}, expected 5"
        assert gid == "6", f"gid was {gid}, expected 6"

        user = machine.succeed("stat -c %U /etc/with_ownership2").strip()
        group = machine.succeed("stat -c %G /etc/with_ownership2").strip()
        assert user == "nobody", f"user was {user}, expected nobody"
        assert group == "users", f"group was {group}, expected users"

        # Verify tmpfiles directories
        machine.succeed("test -d /var/tmp/system-manager")
        machine.succeed("test -d /var/tmp/sample")

        # Verify tmpfiles.d configurations
        machine.succeed("test -f /etc/tmpfiles.d/sample.conf")
        machine.succeed("test -f /etc/tmpfiles.d/00-system-manager.conf")
      '';
  };
}

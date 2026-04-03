{
  forEachDistro,
  system,
  system-manager-v1-1-0,
  ...
}:

forEachDistro "state-v0-v1-migration-deactivate" (
  let
    module = {
      environment.etc = {
        "a/bar" = {
          text = "bar";
          mode = "0700";
          user = "user";
          group = "root";
        };
        "a/link" = {
          text = "link";
          mode = "symlink";
        };
        "b/bar" = {
          text = "bar";
          mode = "0700";
          user = "user";
          group = "root";
          replaceExisting = true;
        };
        "b/link" = {
          text = "link";
          mode = "symlink";
          replaceExisting = true;
        };
      };
    };
    v0TopLevel = system-manager-v1-1-0.lib.makeSystemConfig {
      modules = [
        module
        {
          nixpkgs.hostPlatform = system;
          system-manager.allowAnyDistro = true;
        }

      ];
    };

  in
  {
    modules = [
      module
    ];
    extraPathsToRegister = [ v0TopLevel ];
    testScriptFunction =
      { toplevel, hostPkgs, ... }:
      ''
        # Start the container
        start_all()

        # Wait for systemd to be ready
        machine.wait_for_unit("multi-user.target")
        machine.execute('mkdir -p /etc/b')
        machine.execute('echo "tobackup" > /etc/b/link')
        machine.execute('echo "tobackup" > /etc/b/bar')

        def check_file(path, content):
            file = machine.file(path)
            assert file.exists, f"{path} should exist"
            assert file.is_file, f"{path} should be a file"
            assert file.contains(content), f"{path} should contain {content}"

        # Let's activate the profile with a v0 state file (using an old system-manager checkout)
        machine.succeed("${v0TopLevel}/bin/activate")
        with subtest("Verify correct files are created"):
            check_file("/etc/a/bar", "bar")
            check_file("/etc/a/link", "link")
            check_file("/etc/b/bar", "bar")
            check_file("/etc/b/link", "link")

        # Let's try to deactivate the machine with the new binary, making sure the state migration works.
        machine.succeed("${toplevel}/bin/deactivate")
        with subtest("v1 deactivation restores the backups from a v0 generated state"):
            machine.succeed("test -f /etc/b/bar")
            machine.succeed("test -f /etc/b/link")
            check_file("/etc/b/bar", "tobackup")
            check_file("/etc/b/link", "tobackup")
      '';
  }
)

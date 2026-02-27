{
  forEachDistro,
  system,
  system-manager-v1-1-0,
  ...
}:

forEachDistro "state-v0-v1-migration-activate" (
  let
    module = {
      # Required for v1.1.0.
      nix.enable = false;

      environment.etc = {
        "a/bar" = {
          text = "bar";
          mode = "0700";
          user = "root";
          group = "root";
        };
        "a/link" = {
          text = "link";
          mode = "symlink";
        };
        "b/bar" = {
          text = "bar";
          mode = "0700";
          user = "root";
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
        machine.wait_for_unit("system-manager.target")

        with subtest("Verify correct files are created"):
            check_file("/etc/a/bar", "bar")
            check_file("/etc/a/link", "link")
            check_file("/etc/b/bar", "bar")
            check_file("/etc/b/link", "link")

        # Let's try to deactivate the machine with the new binary, making sure the state migration works.
        machine.succeed("${toplevel}/bin/activate")

        # Check the state backup works
        backup = machine.file("/var/lib/system-manager/state/system-manager-state.json.v0back")
        assert backup.exists, "the v0 state should be backed up"

        with subtest("v1 activation keeps the file and migrate the state to v1"):
            check_file("/etc/a/bar", "bar")
            check_file("/etc/a/link", "link")
            check_file("/etc/b/bar", "bar")
            check_file("/etc/b/link", "link")

        with subtest("Check state content and make sure it's correctly migrated"):
            # Test state
            import json
            file = machine.file("/var/lib/system-manager/state/system-manager-state.json")
            state = json.loads(file.content_string)
            files = state['fileTree']['files']
            assert "/etc/a/bar" in files, "/etc/a/bar should appear in the state as a non backup file"
            assert "/etc/a/link" in files, "/etc/a/link should appear in the state as a non backup file"
            assert not ("/etc/b/bar" in files), "/etc/b/bar should be a backup and not appear in files"
            assert not ("/etc/b/link" in files), "/etc/b/link should be a backup and not appear in files"
            backups = state['fileTree']['backedUpFiles']
            assert "/etc/b/bar" in backups, "/etc/b/bar should appear in the state as a backup file"
            assert "/etc/b/link" in backups, "/etc/b/link should appear in the state as a backup file"
            assert not ("/etc/a/bar" in backups), "/etc/a/bar should not appear in backups"
            assert not ("/etc/a/link" in backups), "/etc/a/link should not appear in backups"
      '';
  }
)
